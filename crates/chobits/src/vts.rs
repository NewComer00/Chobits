//! VTS expression player — discovers model hotkeys and triggers them via the VTube Studio API.

use crate::config::VtsConfig;
use rand::seq::IndexedRandom;
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, oneshot, Mutex};
use vtubestudio::data::{
    AnimationEventType, ApiStateRequest, Event, EventSubscriptionRequest, Hotkey, HotkeyAction,
    HotkeyTriggerRequest, HotkeysInCurrentModelRequest, ModelAnimationEvent,
    ModelAnimationEventConfig,
};
use vtubestudio::{Client, ClientEvent};

const EXPRESSION_WAIT_MS: u64 = 2000;
/// Timer loop for hotkeys without animation metadata (expressions, unknown).
const TIMER_LOOP_INTERVAL_MS: u64 = 4000;
/// Fallback when no ModelAnimationEvent Start/End arrives in time.
const MOTION_FALLBACK_MS: u64 = 3000;
const CONNECT_RETRY_INTERVAL: Duration = Duration::from_secs(2);
const MOTION_EVENT_TIMEOUT: Duration = Duration::from_secs(30);
const ANIM_EVENT_BUFFER: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HotkeyKind {
    Expression,
    Motion,
    Idle,
}

#[derive(Debug, Clone)]
struct ExpressionEntry {
    hotkey_id: String,
    animation_file: String,
    kind: HotkeyKind,
    can_loop: bool,
    duration_ms: u64,
    loop_interval_ms: u64,
    fallback_ms: u64,
}

#[derive(Clone)]
pub struct VtsPlayer {
    default_loop_key: String,
    alias_allow: Arc<HashMap<String, Vec<String>>>,
    client: Arc<Mutex<Client>>,
    cache: Arc<HashMap<String, ExpressionEntry>>,
    anim_events: broadcast::Sender<ModelAnimationEvent>,
}

impl VtsPlayer {
    pub async fn new(vts: &VtsConfig) -> Result<Self, String> {
        let stored_token = load_auth_token(&vts.auth_token_path);
        let (client, events) = connect_with_retry(vts, stored_token).await?;
        let client = Arc::new(Mutex::new(client));
        let (anim_tx, _anim_rx) = broadcast::channel(ANIM_EVENT_BUFFER);

        spawn_vts_event_loop(
            client.clone(),
            events,
            vts.auth_token_path.clone(),
            anim_tx.clone(),
        );
        subscribe_animation_events(&client).await?;

        let (cache, discovery_order) = {
            let client_guard = client.lock().await;
            discover_expressions(&client_guard).await?
        };
        let alias_allow = build_alias_maps(&cache, vts);
        let fallback_key = pick_fallback_key(&cache);
        let default_loop_key =
            pick_default_loop_key(&cache, &discovery_order, &fallback_key, &alias_allow);

        let mut names: Vec<String> = cache.keys().cloned().collect();
        names.sort();
        println!(
            "[vts] Discovered {} hotkeys: {}",
            names.len(),
            names.join(", ")
        );
        if !alias_allow.is_empty() {
            let mut labels: Vec<String> = alias_allow.keys().cloned().collect();
            labels.sort();
            println!("[vts] LLM aliases: {}", labels.join(", "));
            if !alias_covers_key(&default_loop_key, &alias_allow) {
                eprintln!(
                    "[vts] Default loop hotkey {:?} is not listed in any alias table",
                    default_loop_key
                );
            }
        } else {
            eprintln!("[vts] No aliases configured — LLM will not receive expression keys");
        }

        Ok(Self {
            default_loop_key,
            alias_allow: Arc::new(alias_allow),
            client,
            cache: Arc::new(cache),
            anim_events: anim_tx,
        })
    }

    /// Labels sent to the LLM — only configured aliases, never raw hotkey keys.
    pub fn llm_aliases(&self) -> Vec<String> {
        llm_alias_list(&self.alias_allow)
    }

    /// Default alias for the resting loop (prefers configured `idle` alias when present).
    pub fn default_expression_label(&self) -> String {
        if self.alias_allow.contains_key("idle") {
            return "idle".to_string();
        }
        default_alias_for_key(&self.default_loop_key, &self.alias_allow)
    }

    /// Name for `play_looping` — uses the alias when it maps to multiple hotkeys.
    pub fn default_loop_name(&self) -> String {
        loop_name_for_key(&self.default_loop_key, &self.alias_allow)
    }

    pub fn resolve_expression(&self, name: &str) -> String {
        resolve_llm_alias(name, &self.alias_allow, &self.default_loop_key)
    }

    pub async fn play(&self, name: &str, cancel_rx: oneshot::Receiver<()>) {
        let entry = self.load_named(name);
        if entry.hotkey_id.is_empty() {
            eprintln!("[vts] Unknown hotkey for {:?}", name);
            return;
        }
        println!(
            "[vts] play {:?} -> {} ({})",
            name, entry.hotkey_id, entry.animation_file
        );
        self.trigger_hotkey(&entry.hotkey_id).await;
        if entry.animation_file.is_empty()
            || !matches!(entry.kind, HotkeyKind::Motion | HotkeyKind::Idle)
        {
            let _ = self.wait(entry.duration_ms, cancel_rx).await;
            return;
        }

        let mut anim_rx = self.anim_events.subscribe();
        let (outcome, _cancel_rx) = wait_for_motion_end(
            &mut anim_rx,
            &entry.animation_file,
            cancel_rx,
            MOTION_EVENT_TIMEOUT,
            entry.fallback_ms,
        )
        .await;
        match outcome {
            MotionWaitOutcome::Completed => {
                println!("[vts] motion finished: {}", entry.animation_file);
            }
            MotionWaitOutcome::Fallback => {
                eprintln!(
                    "[vts] ModelAnimationEvent timeout for {:?}; used fallback timer",
                    entry.animation_file
                );
            }
            MotionWaitOutcome::Cancelled => {}
        }
    }

    pub async fn play_looping(&self, name: &str, cancel_rx: oneshot::Receiver<()>) {
        let mut name = name.to_string();
        if self.load_named(&name).hotkey_id.is_empty() {
            if name == "thinking" {
                name = waiting_loop_label(&self.cache, &self.alias_allow, &self.default_loop_key);
                if self.load_named(&name).hotkey_id.is_empty() {
                    eprintln!("[vts] Unknown hotkey for {:?} and default loop", name);
                    return;
                }
            } else {
                eprintln!("[vts] Unknown hotkey for {:?}", name);
                return;
            }
        }

        let key = self.resolve_key(&name);
        let entry = self.load_named(&name);
        if entry.hotkey_id.is_empty() {
            eprintln!("[vts] Unknown hotkey for {:?}", name);
            return;
        }

        if is_idle_loop_target(&key, &entry, &self.alias_allow) {
            // Live2D loops idle in-engine; re-triggering does not emit fresh events.
            println!(
                "[vts] idle loop {:?} -> {} ({})",
                name, entry.hotkey_id, entry.animation_file
            );
            self.trigger_hotkey(&entry.hotkey_id).await;
            let _ = cancel_rx.await;
            return;
        }

        self.play_looping_motion(&name, cancel_rx).await;
    }

    async fn play_looping_motion(&self, name: &str, mut cancel_rx: oneshot::Receiver<()>) {
        loop {
            let entry = self.load_named(name);
            if entry.hotkey_id.is_empty() {
                eprintln!("[vts] Unknown hotkey for {:?}", name);
                return;
            }
            self.trigger_hotkey(&entry.hotkey_id).await;
            if entry.animation_file.is_empty()
                || !matches!(entry.kind, HotkeyKind::Motion | HotkeyKind::Idle)
            {
                match self.wait(entry.loop_interval_ms, cancel_rx).await {
                    WaitOutcome::Cancelled => return,
                    WaitOutcome::Elapsed(next_rx) => cancel_rx = next_rx,
                }
                continue;
            }

            let mut anim_rx = self.anim_events.subscribe();
            let (outcome, next_cancel) = wait_for_motion_end(
                &mut anim_rx,
                &entry.animation_file,
                cancel_rx,
                MOTION_EVENT_TIMEOUT,
                entry.fallback_ms,
            )
            .await;
            cancel_rx = next_cancel;
            match outcome {
                MotionWaitOutcome::Cancelled => return,
                MotionWaitOutcome::Completed | MotionWaitOutcome::Fallback => {}
            }
        }
    }

    fn resolve_key(&self, name: &str) -> String {
        let key = normalize_lookup_key(name);
        if self.cache.contains_key(&key) {
            return key;
        }
        self.resolve_expression(name)
    }

    fn load_named(&self, name: &str) -> ExpressionEntry {
        let key = normalize_lookup_key(name);
        let key = if self.cache.contains_key(&key) {
            key
        } else {
            self.resolve_expression(name)
        };
        self.cache.get(&key).cloned().unwrap_or(ExpressionEntry {
            hotkey_id: String::new(),
            animation_file: String::new(),
            kind: HotkeyKind::Expression,
            can_loop: true,
            duration_ms: EXPRESSION_WAIT_MS,
            loop_interval_ms: TIMER_LOOP_INTERVAL_MS,
            fallback_ms: EXPRESSION_WAIT_MS,
        })
    }

    async fn trigger_hotkey(&self, hotkey_id: &str) {
        if hotkey_id.is_empty() {
            return;
        }
        let req = HotkeyTriggerRequest {
            hotkey_id: hotkey_id.to_string(),
            item_instance_id: None,
        };
        let mut client = self.client.lock().await;
        if let Err(e) = client.send(&req).await {
            eprintln!("[vts] HotkeyTriggerRequest failed: {e}");
        }
    }

    async fn wait(&self, duration_ms: u64, cancel_rx: oneshot::Receiver<()>) -> WaitOutcome {
        wait_duration(duration_ms, cancel_rx).await
    }
}

enum WaitOutcome {
    Cancelled,
    Elapsed(oneshot::Receiver<()>),
}

fn llm_alias_list(alias_allow: &HashMap<String, Vec<String>>) -> Vec<String> {
    let mut aliases: Vec<String> = alias_allow.keys().cloned().collect();
    aliases.sort();
    aliases
}

fn default_alias_for_key(key: &str, alias_allow: &HashMap<String, Vec<String>>) -> String {
    alias_allow
        .iter()
        .find_map(|(alias, keys)| keys.iter().any(|k| k == key).then_some(alias.clone()))
        .unwrap_or_else(|| {
            if alias_allow.contains_key("idle") {
                return "idle".to_string();
            }
            llm_alias_list(alias_allow)
                .into_iter()
                .next()
                .unwrap_or_default()
        })
}

fn alias_for_key(key: &str, alias_allow: &HashMap<String, Vec<String>>) -> Option<String> {
    alias_allow
        .iter()
        .find_map(|(alias, keys)| keys.iter().any(|k| k == key).then_some(alias.clone()))
}

fn loop_name_for_key(key: &str, alias_allow: &HashMap<String, Vec<String>>) -> String {
    if let Some(alias) = alias_for_key(key, alias_allow) {
        if alias_allow.get(&alias).is_some_and(|keys| keys.len() > 1) {
            return alias;
        }
    }
    key.to_string()
}

fn name_resolves(
    name: &str,
    cache: &HashMap<String, ExpressionEntry>,
    alias_allow: &HashMap<String, Vec<String>>,
    default_loop_key: &str,
) -> bool {
    let slug = normalize_lookup_key(name);
    if cache
        .get(&slug)
        .is_some_and(|entry| !entry.hotkey_id.is_empty())
    {
        return true;
    }
    if alias_allow.contains_key(&slug) {
        let key = resolve_llm_alias(name, alias_allow, default_loop_key);
        return cache
            .get(&key)
            .is_some_and(|entry| !entry.hotkey_id.is_empty());
    }
    false
}

fn waiting_loop_label(
    cache: &HashMap<String, ExpressionEntry>,
    alias_allow: &HashMap<String, Vec<String>>,
    default_loop_key: &str,
) -> String {
    if name_resolves("thinking", cache, alias_allow, default_loop_key) {
        return "thinking".to_string();
    }
    loop_name_for_key(default_loop_key, alias_allow)
}

fn pick_allow_list_key(keys: &[String]) -> String {
    if keys.len() == 1 {
        return keys[0].clone();
    }
    keys.choose(&mut rand::rng()).cloned().unwrap_or_default()
}

fn resolve_llm_alias(
    name: &str,
    alias_allow: &HashMap<String, Vec<String>>,
    default_loop_key: &str,
) -> String {
    let key = normalize_lookup_key(name);
    if let Some(targets) = alias_allow.get(&key) {
        return pick_allow_list_key(targets);
    }
    if !alias_allow.is_empty() {
        eprintln!(
            "[vts] Unknown alias {:?}; falling back to hotkey {:?}",
            name, default_loop_key
        );
    }
    default_loop_key.to_string()
}

async fn discover_expressions(
    client: &Client,
) -> Result<(HashMap<String, ExpressionEntry>, Vec<String>), String> {
    let mut client = client.clone();
    let resp = client
        .send(&HotkeysInCurrentModelRequest::default())
        .await
        .map_err(|e| format!("HotkeysInCurrentModelRequest failed: {e}"))?;

    if !resp.model_loaded {
        return Err("VTS has no model loaded — start live-ascii with a model first".into());
    }

    println!("[vts] Querying hotkeys for model {:?}", resp.model_name);

    let mut cache = HashMap::new();
    let mut discovery_order = Vec::new();
    for hk in resp.available_hotkeys {
        if let Some((key, entry)) = hotkey_to_entry(&hk) {
            if cache.contains_key(&key) {
                eprintln!(
                    "[vts] Skipping duplicate key {:?} (hotkey {:?})",
                    key, hk.name
                );
                continue;
            }
            discovery_order.push(key.clone());
            cache.insert(key, entry);
        }
    }

    if cache.is_empty() {
        return Err(format!(
            "No motion/expression hotkeys found on model {:?}",
            resp.model_name
        ));
    }

    Ok((cache, discovery_order))
}

fn hotkey_to_entry(hk: &Hotkey) -> Option<(String, ExpressionEntry)> {
    let is_motion = hk.type_ == HotkeyAction::TriggerAnimation;
    let is_expression = hk.type_ == HotkeyAction::ToggleExpression;
    let is_idle = hk.type_ == HotkeyAction::ChangeIdleAnimation;

    if !is_motion && !is_expression && !is_idle {
        return None;
    }

    let key = expression_key(hk, is_motion)?;
    let kind = if is_idle {
        HotkeyKind::Idle
    } else if is_motion {
        HotkeyKind::Motion
    } else {
        HotkeyKind::Expression
    };
    let can_loop = is_idle || is_motion;
    let wait_ms = if is_motion {
        MOTION_FALLBACK_MS
    } else {
        EXPRESSION_WAIT_MS
    };

    Some((
        key.clone(),
        ExpressionEntry {
            hotkey_id: hk.hotkey_id.clone(),
            animation_file: hk.file.clone(),
            kind,
            can_loop,
            duration_ms: wait_ms,
            loop_interval_ms: if can_loop {
                TIMER_LOOP_INTERVAL_MS
            } else {
                wait_ms
            },
            fallback_ms: wait_ms,
        },
    ))
}

fn expression_key(hk: &Hotkey, is_motion: bool) -> Option<String> {
    if is_motion {
        if let Some(label) = normalize_label(&hk.name) {
            if label_key_is_useful(&label) {
                return Some(label);
            }
        }
        file_stem_key(&hk.file).or_else(|| normalize_label(&hk.name))
    } else {
        normalize_label(&hk.name).or_else(|| file_stem_key(&hk.file))
    }
    .filter(|k| !k.is_empty())
}

fn label_key_is_useful(slug: &str) -> bool {
    slug.chars().any(|c| c.is_alphabetic())
}

fn normalize_label(name: &str) -> Option<String> {
    let mut s = name.trim().to_string();
    for prefix in [
        "Motion:",
        "motion:",
        "Expression:",
        "expression:",
        "Anim:",
        "Animation:",
        "Idle:",
        "idle:",
    ] {
        if let Some(rest) = s.strip_prefix(prefix) {
            s = rest.trim().to_string();
            break;
        }
    }
    let slug = slugify(&s);
    if slug.is_empty() {
        None
    } else {
        Some(slug)
    }
}

fn file_stem_key(file: &str) -> Option<String> {
    if file.is_empty() {
        return None;
    }
    let mut stem = Path::new(file).file_stem()?.to_str()?.to_string();
    while stem.contains('.') {
        stem = Path::new(&stem).file_stem()?.to_str()?.to_string();
    }
    let slug = slugify(&stem);
    if slug.is_empty() {
        None
    } else {
        Some(slug)
    }
}

fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut prev_underscore = false;
    for c in s.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            prev_underscore = false;
        } else if !prev_underscore && !out.is_empty() {
            out.push('_');
            prev_underscore = true;
        }
    }
    out.trim_matches('_').to_string()
}

fn normalize_lookup_key(name: &str) -> String {
    slugify(name)
}

fn lookup_cache_key(cache: &HashMap<String, ExpressionEntry>, target: &str) -> Option<String> {
    let direct = normalize_lookup_key(target);
    if cache.contains_key(&direct) {
        return Some(direct);
    }
    normalize_label(target).filter(|key| cache.contains_key(key))
}

fn loopable_keys_in_order(
    cache: &HashMap<String, ExpressionEntry>,
    discovery_order: &[String],
) -> Vec<String> {
    discovery_order
        .iter()
        .filter(|key| cache.get(*key).is_some_and(|entry| entry.can_loop))
        .cloned()
        .collect()
}

fn sorted_keys(cache: &HashMap<String, ExpressionEntry>) -> Vec<String> {
    let mut keys: Vec<String> = cache.keys().cloned().collect();
    keys.sort();
    keys
}

fn alias_covers_key(key: &str, alias_allow: &HashMap<String, Vec<String>>) -> bool {
    alias_allow
        .values()
        .any(|targets| targets.iter().any(|target| target == key))
}

fn idle_alias_contains_key(key: &str, alias_allow: &HashMap<String, Vec<String>>) -> bool {
    alias_allow
        .get("idle")
        .is_some_and(|targets| targets.iter().any(|target| target == key))
}

/// Idle hotkeys loop inside Live2D — re-triggering them does not emit fresh
/// animation events, so wait for cancel instead of Start/End cycles.
fn is_idle_loop_target(
    key: &str,
    entry: &ExpressionEntry,
    alias_allow: &HashMap<String, Vec<String>>,
) -> bool {
    entry.kind == HotkeyKind::Idle || idle_alias_contains_key(key, alias_allow)
}

fn pick_fallback_key(cache: &HashMap<String, ExpressionEntry>) -> String {
    sorted_keys(cache)
        .into_iter()
        .next()
        .expect("discover_expressions ensures a non-empty cache")
}

fn loopable_idle_alias_keys(
    cache: &HashMap<String, ExpressionEntry>,
    alias_allow: &HashMap<String, Vec<String>>,
) -> Vec<String> {
    alias_allow
        .get("idle")
        .into_iter()
        .flat_map(|keys| keys.iter())
        .filter(|key| cache.get(*key).is_some_and(|entry| entry.can_loop))
        .cloned()
        .collect()
}

fn pick_default_loop_key(
    cache: &HashMap<String, ExpressionEntry>,
    discovery_order: &[String],
    fallback: &str,
    alias_allow: &HashMap<String, Vec<String>>,
) -> String {
    if let Some(key) = loopable_idle_alias_keys(cache, alias_allow)
        .into_iter()
        .next()
    {
        return key;
    }
    loopable_keys_in_order(cache, discovery_order)
        .into_iter()
        .next()
        .unwrap_or_else(|| fallback.to_string())
}

enum AliasKind {
    Expression,
    Motion,
}

fn build_alias_maps(
    cache: &HashMap<String, ExpressionEntry>,
    vts: &VtsConfig,
) -> HashMap<String, Vec<String>> {
    let mut alias_allow = HashMap::new();

    for (label, targets) in &vts.expression_alias {
        register_alias(
            cache,
            &mut alias_allow,
            label,
            &targets.keys(),
            AliasKind::Expression,
            "expression",
        );
    }
    for (label, targets) in &vts.motion_alias {
        register_alias(
            cache,
            &mut alias_allow,
            label,
            &targets.keys(),
            AliasKind::Motion,
            "motion",
        );
    }

    alias_allow
}

fn register_alias(
    cache: &HashMap<String, ExpressionEntry>,
    alias_allow: &mut HashMap<String, Vec<String>>,
    label: &str,
    targets: &[String],
    expected_kind: AliasKind,
    kind_name: &str,
) {
    let alias = normalize_lookup_key(label);
    if alias.is_empty() {
        eprintln!("[vts] Skipping empty {kind_name} alias");
        return;
    }
    if alias_allow.contains_key(&alias) {
        eprintln!("[vts] Duplicate {kind_name} alias {label:?}");
        return;
    }

    let mut keys = Vec::new();
    for target in targets {
        let Some(key) = lookup_cache_key(cache, target) else {
            eprintln!("[vts] {kind_name} alias {label:?} targets unknown hotkey {target:?}");
            continue;
        };
        let entry = &cache[&key];
        let kind_ok = match expected_kind {
            AliasKind::Expression => entry.kind == HotkeyKind::Expression,
            AliasKind::Motion => matches!(entry.kind, HotkeyKind::Motion | HotkeyKind::Idle),
        };
        if !kind_ok {
            eprintln!("[vts] {kind_name} alias {label:?} targets {target:?} (wrong hotkey type)");
            continue;
        }
        if !keys.contains(&key) {
            keys.push(key);
        }
    }

    if keys.is_empty() {
        eprintln!("[vts] {kind_name} alias {label:?} has no valid hotkeys");
        return;
    }

    alias_allow.insert(alias, keys);
}

fn load_auth_token(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn save_auth_token(path: &Path, token: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create {:?}: {e}", parent))?;
    }
    std::fs::write(path, token).map_err(|e| format!("Failed to write {:?}: {e}", path))
}

fn spawn_vts_event_loop(
    client: Arc<Mutex<Client>>,
    mut events: vtubestudio::ClientEventStream,
    token_path: PathBuf,
    anim_tx: broadcast::Sender<ModelAnimationEvent>,
) {
    tokio::spawn(async move {
        while let Some(event) = events.next().await {
            match event {
                ClientEvent::Connected => {
                    while subscribe_animation_events(&client).await.is_err() {
                        tokio::time::sleep(CONNECT_RETRY_INTERVAL).await;
                    }
                }
                ClientEvent::NewAuthToken(token) => {
                    if let Err(e) = save_auth_token(&token_path, &token) {
                        eprintln!("[vts] Failed to save auth token: {e}");
                    }
                }
                ClientEvent::Api(Event::ModelAnimation(ev)) => {
                    let _ = anim_tx.send(ev);
                }
                _ => {}
            }
        }
    });
}

async fn subscribe_animation_events(client: &Arc<Mutex<Client>>) -> Result<(), String> {
    let req = EventSubscriptionRequest::subscribe(&ModelAnimationEventConfig {
        ignore_live2d_items: true,
        ignore_idle_animations: false,
    })
    .map_err(|e| format!("ModelAnimationEventConfig: {e}"))?;

    let mut client = client.lock().await;
    let resp = client
        .send(&req)
        .await
        .map_err(|e| format!("EventSubscriptionRequest failed: {e}"))?;
    println!(
        "[vts] Subscribed to {} animation event(s)",
        resp.subscribed_event_count
    );
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
enum MotionWaitOutcome {
    Completed,
    Cancelled,
    Fallback,
}

fn drain_animation_events(rx: &mut broadcast::Receiver<ModelAnimationEvent>) {
    loop {
        match rx.try_recv() {
            Ok(_) => {}
            Err(broadcast::error::TryRecvError::Empty) => break,
            Err(broadcast::error::TryRecvError::Lagged(_)) => {}
            Err(broadcast::error::TryRecvError::Closed) => break,
        }
    }
}

fn animation_length_ms(length_secs: f64) -> u64 {
    (length_secs * 1000.0).round().max(100.0) as u64
}

async fn wait_for_motion_end(
    rx: &mut broadcast::Receiver<ModelAnimationEvent>,
    expected_file: &str,
    cancel_rx: oneshot::Receiver<()>,
    timeout: Duration,
    fallback_ms: u64,
) -> (MotionWaitOutcome, oneshot::Receiver<()>) {
    drain_animation_events(rx);

    let deadline = tokio::time::Instant::now() + timeout;
    let mut cancel_rx = cancel_rx;
    let mut saw_matching_start = false;
    let mut event_fallback_ms = fallback_ms;

    loop {
        tokio::select! {
            _ = &mut cancel_rx => return (MotionWaitOutcome::Cancelled, cancel_rx),
            msg = tokio::time::timeout_at(deadline, rx.recv()) => {
                match msg {
                    Err(_) | Ok(Err(broadcast::error::RecvError::Closed)) => {
                        return finish_after_fallback(event_fallback_ms, cancel_rx).await;
                    }
                    Ok(Err(broadcast::error::RecvError::Lagged(_))) => continue,
                    Ok(Ok(ev)) => {
                        if !animation_matches(&ev.animation_name, expected_file) {
                            continue;
                        }
                        if ev.animation_event_type == AnimationEventType::Start {
                            saw_matching_start = true;
                            event_fallback_ms = animation_length_ms(ev.animation_length);
                            continue;
                        }
                        if ev.animation_event_type == AnimationEventType::End && saw_matching_start {
                            return (MotionWaitOutcome::Completed, cancel_rx);
                        }
                    }
                }
            }
        }
    }
}

async fn finish_after_fallback(
    duration_ms: u64,
    cancel_rx: oneshot::Receiver<()>,
) -> (MotionWaitOutcome, oneshot::Receiver<()>) {
    let (cancelled, cancel_rx) = wait_or_cancel(duration_ms, cancel_rx).await;
    if cancelled {
        (MotionWaitOutcome::Cancelled, cancel_rx)
    } else {
        (MotionWaitOutcome::Fallback, cancel_rx)
    }
}

async fn wait_or_cancel(
    duration_ms: u64,
    mut cancel_rx: oneshot::Receiver<()>,
) -> (bool, oneshot::Receiver<()>) {
    tokio::select! {
        _ = &mut cancel_rx => (true, cancel_rx),
        _ = tokio::time::sleep(Duration::from_millis(duration_ms)) => (false, cancel_rx),
    }
}

fn animation_matches(animation_name: &str, expected_file: &str) -> bool {
    if animation_name == expected_file
        || animation_name.ends_with(expected_file)
        || expected_file.ends_with(animation_name)
    {
        return true;
    }
    file_basename(animation_name) == file_basename(expected_file)
}

fn file_basename(path: &str) -> &str {
    Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
}

async fn wait_duration(duration_ms: u64, mut cancel_rx: oneshot::Receiver<()>) -> WaitOutcome {
    tokio::select! {
        _ = &mut cancel_rx => WaitOutcome::Cancelled,
        _ = tokio::time::sleep(Duration::from_millis(duration_ms)) => {
            WaitOutcome::Elapsed(cancel_rx)
        }
    }
}

async fn connect_with_retry(
    vts: &VtsConfig,
    stored_token: Option<String>,
) -> Result<(Client, vtubestudio::ClientEventStream), String> {
    let timeout = Duration::from_secs(vts.connect_timeout_secs);
    let started = std::time::Instant::now();

    loop {
        let (client, events) = Client::builder()
            .url(vts.url.clone())
            .authentication(
                vts.plugin_name.clone(),
                vts.developer.clone(),
                None::<Cow<'static, str>>,
            )
            .auth_token(stored_token.clone())
            .build_tungstenite();

        let mut probe = client.clone();
        match probe.send(&ApiStateRequest {}).await {
            Ok(state) if state.active => return Ok((client, events)),
            Ok(_) => {
                if started.elapsed() >= timeout {
                    return Err("VTS API is not active".into());
                }
                eprintln!("[vts] Waiting for VTS at {} (API not active)", vts.url);
                tokio::time::sleep(CONNECT_RETRY_INTERVAL).await;
            }
            Err(e) => {
                if started.elapsed() >= timeout {
                    return Err(format!(
                        "VTS connection failed after {}s: {e}",
                        vts.connect_timeout_secs
                    ));
                }
                eprintln!("[vts] Waiting for VTS at {} ({e})", vts.url);
                tokio::time::sleep(CONNECT_RETRY_INTERVAL).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vtubestudio::data::EnumString;

    const TAP_MOTION: &str = "motion/hiyori_tap.motion3.json";

    fn sample_hotkey(name: &str, action: HotkeyAction, file: &str) -> Hotkey {
        Hotkey {
            name: name.into(),
            type_: EnumString::new(action),
            file: file.into(),
            hotkey_id: format!("id-{name}"),
            description: None,
            key_combination: vec![],
            on_screen_button_id: -1,
        }
    }

    fn test_entry(kind: HotkeyKind, can_loop: bool) -> ExpressionEntry {
        ExpressionEntry {
            hotkey_id: "test-hotkey".into(),
            animation_file: String::new(),
            kind,
            can_loop,
            duration_ms: 2000,
            loop_interval_ms: if can_loop { 4000 } else { 2000 },
            fallback_ms: 2000,
        }
    }

    fn motion_event_channel() -> (
        broadcast::Sender<ModelAnimationEvent>,
        broadcast::Receiver<ModelAnimationEvent>,
    ) {
        let (tx, rx) = broadcast::channel(16);
        (tx, rx)
    }

    async fn send_motion_lifecycle(
        tx: broadcast::Sender<ModelAnimationEvent>,
        animation_name: &str,
        is_idle: bool,
    ) {
        let _ = tx.send(sample_animation_event(
            AnimationEventType::Start,
            animation_name,
            is_idle,
        ));
        let _ = tx.send(sample_animation_event(
            AnimationEventType::End,
            animation_name,
            is_idle,
        ));
    }

    fn sample_animation_event(
        event_type: AnimationEventType,
        animation_name: &str,
        is_idle: bool,
    ) -> ModelAnimationEvent {
        ModelAnimationEvent {
            animation_event_type: EnumString::new(event_type),
            animation_event_time: 0.0,
            animation_event_data: String::new(),
            animation_name: animation_name.to_string(),
            animation_length: 1.5,
            is_idle_animation: is_idle,
            model_id: String::new(),
            model_name: String::new(),
            is_live2d_item: false,
        }
    }

    #[tokio::test]
    async fn wait_for_motion_end_completes_on_matching_end() {
        let (tx, mut rx) = motion_event_channel();
        let tx_bg = tx.clone();
        let motion = TAP_MOTION.to_string();
        tokio::spawn(async move {
            send_motion_lifecycle(tx_bg, &motion, false).await;
        });

        let (_cancel_tx, cancel_rx) = oneshot::channel();
        let (outcome, _cancel_rx) =
            wait_for_motion_end(&mut rx, TAP_MOTION, cancel_rx, Duration::from_secs(1), 100).await;
        assert_eq!(outcome, MotionWaitOutcome::Completed);
    }

    #[tokio::test]
    async fn wait_for_motion_end_completes_on_idle_end() {
        let (tx, mut rx) = motion_event_channel();
        let tx_bg = tx.clone();
        let motion = "motion/hiyori_m01.motion3.json".to_string();
        tokio::spawn(async move {
            send_motion_lifecycle(tx_bg, &motion, true).await;
        });

        let (_cancel_tx, cancel_rx) = oneshot::channel();
        let (outcome, _) = wait_for_motion_end(
            &mut rx,
            "motion/hiyori_m01.motion3.json",
            cancel_rx,
            Duration::from_secs(1),
            100,
        )
        .await;
        assert_eq!(outcome, MotionWaitOutcome::Completed);
    }

    #[tokio::test]
    async fn wait_for_motion_end_ignores_stale_end_without_start() {
        let (tx, mut rx) = motion_event_channel();
        let _ = tx.send(sample_animation_event(
            AnimationEventType::End,
            TAP_MOTION,
            false,
        ));

        let (_cancel_tx, cancel_rx) = oneshot::channel();
        let (outcome, _) =
            wait_for_motion_end(&mut rx, TAP_MOTION, cancel_rx, Duration::from_millis(10), 1).await;
        assert_eq!(outcome, MotionWaitOutcome::Fallback);
    }

    #[tokio::test]
    async fn wait_for_motion_end_ignores_unrelated_events() {
        let (tx, mut rx) = motion_event_channel();
        let tx_bg = tx.clone();
        let motion = TAP_MOTION.to_string();
        tokio::spawn(async move {
            let tx_other = tx_bg.clone();
            let _ = tx_other.send(sample_animation_event(
                AnimationEventType::End,
                "motion/other.motion3.json",
                false,
            ));
            send_motion_lifecycle(tx_bg, &motion, false).await;
        });

        let (_cancel_tx, cancel_rx) = oneshot::channel();
        let (outcome, _) =
            wait_for_motion_end(&mut rx, TAP_MOTION, cancel_rx, Duration::from_secs(1), 100).await;
        assert_eq!(outcome, MotionWaitOutcome::Completed);
    }

    #[tokio::test]
    async fn wait_for_motion_end_honours_cancel() {
        let (_tx, mut rx) = motion_event_channel();
        let (cancel_tx, cancel_rx) = oneshot::channel();
        cancel_tx.send(()).unwrap();

        let (outcome, _) =
            wait_for_motion_end(&mut rx, TAP_MOTION, cancel_rx, Duration::from_secs(1), 100).await;
        assert_eq!(outcome, MotionWaitOutcome::Cancelled);
    }

    #[tokio::test]
    async fn wait_for_motion_end_falls_back_without_end() {
        let (_tx, mut rx) = motion_event_channel();
        let (_cancel_tx, cancel_rx) = oneshot::channel();
        let (outcome, _) =
            wait_for_motion_end(&mut rx, TAP_MOTION, cancel_rx, Duration::from_millis(10), 1).await;
        assert_eq!(outcome, MotionWaitOutcome::Fallback);
    }

    #[test]
    fn llm_alias_list_returns_only_configured_aliases() {
        let aliases = HashMap::from([
            ("idle".into(), vec!["idle_0".into(), "idle_1".into()]),
            ("wave".into(), vec!["tap_0".into()]),
        ]);
        assert_eq!(
            llm_alias_list(&aliases),
            vec!["idle".to_string(), "wave".to_string()]
        );
        assert!(llm_alias_list(&HashMap::new()).is_empty());
    }

    #[test]
    fn resolve_picks_from_allow_list() {
        let aliases = HashMap::from([("idle".into(), vec!["idle_0".into(), "idle_1".into()])]);
        let resolved = resolve_llm_alias("idle", &aliases, "idle_0");
        assert!(resolved == "idle_0" || resolved == "idle_1");
        assert_eq!(resolve_llm_alias("unknown", &aliases, "idle_0"), "idle_0");
        assert_eq!(
            resolve_llm_alias(
                "wave",
                &HashMap::from([("wave".into(), vec!["tap_0".into()],)]),
                "idle_0"
            ),
            "tap_0"
        );
    }

    #[test]
    fn loop_name_uses_alias_when_allow_list_has_multiple() {
        let aliases = HashMap::from([("idle".into(), vec!["idle_0".into(), "idle_1".into()])]);
        assert_eq!(loop_name_for_key("idle_0", &aliases), "idle");
        assert_eq!(loop_name_for_key("tap_0", &aliases), "tap_0");
    }

    #[test]
    fn default_alias_finds_label_for_any_allow_list_member() {
        let aliases = HashMap::from([("idle".into(), vec!["idle_0".into(), "idle_1".into()])]);
        assert_eq!(default_alias_for_key("idle_1", &aliases), "idle");
    }

    fn loop_test_cache() -> HashMap<String, ExpressionEntry> {
        HashMap::from([
            ("idle_0".into(), test_entry(HotkeyKind::Motion, true)),
            ("idle_1".into(), test_entry(HotkeyKind::Motion, true)),
            ("idle_2".into(), test_entry(HotkeyKind::Motion, true)),
            ("tap_0".into(), test_entry(HotkeyKind::Motion, false)),
            ("flick_body_0".into(), test_entry(HotkeyKind::Motion, false)),
        ])
    }

    #[test]
    fn is_idle_loop_target_matches_idle_kind_and_alias() {
        let entry = test_entry(HotkeyKind::Idle, true);
        let aliases = HashMap::from([("idle".into(), vec!["idle_2".into()])]);
        assert!(is_idle_loop_target("idle_2", &entry, &aliases));
        let motion = test_entry(HotkeyKind::Motion, true);
        assert!(!is_idle_loop_target("tap_0", &motion, &aliases));
        assert!(is_idle_loop_target("idle_2", &motion, &aliases));
    }

    #[test]
    fn waiting_loop_label_tries_thinking_then_default() {
        let mut cache = loop_test_cache();
        cache.insert("thinking".into(), test_entry(HotkeyKind::Motion, false));
        assert_eq!(
            waiting_loop_label(&cache, &HashMap::new(), "idle_0"),
            "thinking"
        );

        let aliases = HashMap::from([("thinking".into(), vec!["tap_0".into()])]);
        assert_eq!(
            waiting_loop_label(&loop_test_cache(), &aliases, "idle_0"),
            "thinking"
        );

        let idle_aliases = HashMap::from([("idle".into(), vec!["idle_0".into(), "idle_1".into()])]);
        assert_eq!(
            waiting_loop_label(&loop_test_cache(), &idle_aliases, "idle_0"),
            "idle"
        );
        assert_eq!(
            waiting_loop_label(&loop_test_cache(), &HashMap::new(), "idle_0"),
            "idle_0"
        );
    }

    #[test]
    fn default_loop_prefers_idle_alias_over_discovery_order() {
        let cache = loop_test_cache();
        let discovery_order = vec!["tap_0".into(), "idle_0".into(), "idle_1".into()];
        let aliases = HashMap::from([(
            "idle".into(),
            vec!["idle_0".into(), "idle_1".into(), "idle_2".into()],
        )]);
        assert_eq!(
            pick_default_loop_key(&cache, &discovery_order, "tap_0", &aliases),
            "idle_0"
        );
    }

    #[test]
    fn animation_matches_equivalent_paths() {
        assert!(animation_matches(
            "motion/hiyori_m01.motion3.json",
            "hiyori_m01.motion3.json"
        ));
        assert!(animation_matches(
            "motion/hiyori_m03.motion3.json",
            "motions/hiyori_m03.motion3.json"
        ));
        assert!(animation_matches(
            "motions/hiyori_tap.motion3.json",
            "motion/hiyori_tap.motion3.json"
        ));
        assert!(!animation_matches(
            "motion/hiyori_m01.motion3.json",
            "motion/hiyori_tap.motion3.json"
        ));
    }

    #[test]
    fn lookup_cache_key_accepts_vts_hotkey_names() {
        let cache = loop_test_cache();
        assert_eq!(
            lookup_cache_key(&cache, "Idle #0").as_deref(),
            Some("idle_0")
        );
        assert_eq!(
            lookup_cache_key(&cache, "Idle #2").as_deref(),
            Some("idle_2")
        );
        assert_eq!(
            lookup_cache_key(&cache, "Flick@Body #0").as_deref(),
            Some("flick_body_0")
        );
    }

    #[test]
    fn hiyori_motion_keys_use_group_labels() {
        let hk = sample_hotkey(
            "Idle #0",
            HotkeyAction::TriggerAnimation,
            "motion/hiyori_m01.motion3.json",
        );
        assert_eq!(expression_key(&hk, true).as_deref(), Some("idle_0"));
    }

    #[test]
    fn weak_motion_labels_use_file_stem() {
        let hk = sample_hotkey(
            " #0",
            HotkeyAction::TriggerAnimation,
            "motions/mtn_02.motion3.json",
        );
        assert_eq!(expression_key(&hk, true).as_deref(), Some("mtn_02"));
    }
}
