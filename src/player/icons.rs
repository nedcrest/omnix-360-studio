//! The player's icons (Lucide-style SVG, drawn white) and the small widget
//! helpers the bars and panels share: hinted icon buttons, labels,
//! separators, the credit line.

use super::*;

/// Two film frames and a plus: give every cut this look.
pub(super) const TO_ALL_CUTS: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="3" width="11" height="8" rx="1"/><rect x="6" y="9" width="11" height="8" rx="1"/><path d="M9 9v2M13 9v2M20 15v6M17 18h6"/></svg>"#;

/// Three sliders (the Effects panel).
pub(super) const EFFECTS: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 4h-7M10 4H3M21 12h-9M8 12H3M21 20h-5M12 20H3M14 2v4M8 10v4M16 18v4"/></svg>"#;

/// Transitions: two frames, a sweep from one into the other.
pub(super) const TRANSITIONS: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="5" width="9" height="14" rx="2"/><rect x="13" y="5" width="9" height="14" rx="2"/><path d="M6 12h12M15 9l3 3-3 3"/></svg>"#;
/// Two strips crossing: one picture dissolving into the next.
pub(super) const CROSSFADE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="6" width="12" height="12" rx="2"/><rect x="10" y="6" width="12" height="12" rx="2" stroke-dasharray="2 2"/></svg>"#;

// Edit icons: scissors (split), a bin (delete cut), arrows (undo, and
// redo below the zoom icons).
pub(super) const SPLIT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="6" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><path d="M20 4 8.1 15.9M14.5 14.5 20 20M8.1 8.1 12 12"/></svg>"#;

pub(super) const DELETE_CUT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18M8 6V4h8v2M19 6l-1 14H6L5 6M10 11v6M14 11v6"/></svg>"#;

pub(super) const UNDO: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 14 4 9l5-5"/><path d="M4 9h10.5a5.5 5.5 0 0 1 0 11H11"/></svg>"#;

/// Timeline zoom icons: a magnifier with a minus or plus, and arrows out to
/// both ends (fit).
pub(super) const ZOOM_OUT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round"><circle cx="10" cy="10" r="7"/><path d="m21 21-6-6M7 10h6"/></svg>"#;

pub(super) const ZOOM_IN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round"><circle cx="10" cy="10" r="7"/><path d="m21 21-6-6M7 10h6M10 7v6"/></svg>"#;

pub(super) const ZOOM_FIT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 5v14M21 5v14M7 12h10M10 9l-3 3 3 3M14 9l3 3-3 3"/></svg>"#;

pub(super) const REDO: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m15 14 5-5-5-5"/><path d="M20 9H9.5a5.5 5.5 0 0 0 0 11H13"/></svg>"#;

/// A pipette: pick a colour from the picture.
pub(super) const PICK: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m2 22 1-1h3l9-9M3 21v-3l9-9"/><path d="m15 6 3.4-3.4a2.1 2.1 0 1 1 3 3L18 9l.4.4a2.1 2.1 0 1 1-3 3l-3.8-3.8a2.1 2.1 0 1 1 3-3z"/></svg>"#;

// Keyframe icons: a diamond with a plus (add or update) or a cross (delete).
pub(super) const KEY_ADD: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linejoin="round"><path d="M10 2 18 10 10 18 2 10z" fill="white"/><path d="M19 15v8M15 19h8" stroke-linecap="round"/></svg>"#;

pub(super) const KEY_DELETE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linejoin="round"><path d="M10 2 18 10 10 18 2 10z"/><path d="m16 16 6 6M22 16l-6 6" stroke-linecap="round"/></svg>"#;

/// A letter A with a minus or a plus: the interface's size.
pub(super) const TEXT_SMALLER: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 19l5-14 5 14M5 14h6M16 12h6"/></svg>"#;

pub(super) const TEXT_BIGGER: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 19l5-14 5 14M5 14h6M16 12h6M19 9v6"/></svg>"#;

/// A speaker with a spark and a plus: add a sound effect.
pub(super) const ADD_SFX: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 9v6h4l5 4V5L7 9z"/><path d="M15 9l2-2M16 12h2"/><path d="M19 15v6M16 18h6"/></svg>"#;

/// A microphone and a plus: add a voice-over.
pub(super) const ADD_VOICE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="5" y="2" width="6" height="11" rx="3"/><path d="M2 10a6 6 0 0 0 12 0M8 16v4"/><path d="M19 13v6M16 16h6"/></svg>"#;
/// A note with a plus: add music.
pub(super) const ADD_MUSIC: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 18V5l10-2v9"/><circle cx="6" cy="18" r="3"/><path d="M19 15v6M16 18h6"/></svg>"#;

/// A page with a plus: a new project.
pub(super) const NEW_PROJECT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7z"/><path d="M14 2v4a2 2 0 0 0 2 2h4M9 15h6M12 18v-6"/></svg>"#;

pub(super) const ADD_OVERLAY: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="14" height="14" rx="2"/><circle cx="8" cy="8" r="1.5"/><path d="M17 12l-4-4-8 8"/><path d="M20 15v6M17 18h6"/></svg>"#;

pub(super) const ADD_BLANK: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="5" width="14" height="10" rx="1" fill="white" fill-opacity="0.35"/><path d="M20 15v6M17 18h6"/></svg>"#;

/// The bars' groups named (down) or not (right).
pub(super) const CHEVRON_DOWN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M6 9l6 6 6-6"/></svg>"#;
pub(super) const CHEVRON_RIGHT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 6l6 6-6 6"/></svg>"#;

/// Snapping on: a magnet; off, crossed out.
pub(super) const SNAP_ON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m6 15-4-4 6.75-6.77a7.79 7.79 0 0 1 11 11L13 22l-4-4 6.39-6.36a2.14 2.14 0 0 0-3-3L6 15"/><path d="m5 8 4 4"/><path d="m12 15 4 4"/></svg>"#;
pub(super) const SNAP_OFF: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" opacity="0.6"><path d="m6 15-4-4 6.75-6.77a7.79 7.79 0 0 1 11 11L13 22l-4-4 6.39-6.36a2.14 2.14 0 0 0-3-3L6 15"/><path d="M2 2l20 20"/></svg>"#;

pub(super) const ADD_TEXT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 7V4h12v3M10 4v16M7 20h6"/><path d="M19 13v8M15 17h8"/></svg>"#;

/// A film strip with a plus: open or add clips.
pub(super) const ADD_CLIP: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="3" width="14" height="18" rx="2"/><path d="M6 3v18M12 3v18M2 8h4M2 16h4M12 8h4M12 16h4M20 9v6M17 12h6"/></svg>"#;

/// An open folder: open a project.
pub(super) const OPEN_PROJECT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2"/></svg>"#;

/// A diskette: save; with a pencil, save as.
pub(super) const SAVE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M15.2 3a2 2 0 0 1 1.4.6l3.8 3.8a2 2 0 0 1 .6 1.4V19a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z"/><path d="M17 21v-7a1 1 0 0 0-1-1H8a1 1 0 0 0-1 1v7M7 3v4a1 1 0 0 0 1 1h7"/></svg>"#;

pub(super) const SAVE_AS: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h10.2a2 2 0 0 1 1.4.6l3.8 3.8a2 2 0 0 1 .6 1.4V10"/><path d="M7 3v4a1 1 0 0 0 1 1h7M7 21v-7a1 1 0 0 1 1-1h4"/><path d="M21.4 15.6a1.98 1.98 0 0 0-2.8-2.8L14 17.4V20.2h2.8z"/></svg>"#;

/// An arrow out of a tray (export) and a crossed circle (cancel a running job).
pub(super) const EXPORT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 15V3M7 8l5-5 5 5"/><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/></svg>"#;

pub(super) const SETTINGS: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/></svg>"#;

pub(super) const FOLDER: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/></svg>"#;

pub(super) const COPY: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/></svg>"#;

pub(super) const CANCEL: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><path d="m15 9-6 6M9 9l6 6"/></svg>"#;

/// Transport: play, pause, a camera (save the view) and a globe (save the
/// whole sphere).
pub(super) const PLAY: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M7 4v16l13-8z" fill="white"/></svg>"#;

pub(super) const PAUSE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="6" y="4" width="4" height="16" rx="1" fill="white"/><rect x="14" y="4" width="4" height="16" rx="1" fill="white"/></svg>"#;

pub(super) const SAVE_VIEW: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14.5 4h-5L7 7H4a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-3z"/><circle cx="12" cy="13" r="3"/></svg>"#;

pub(super) const SAVE_SPHERE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M2 12h20M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/></svg>"#;

/// View: turn back (reset the view) and corners out or in (fullscreen).
pub(super) const RESET_VIEW: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/></svg>"#;

pub(super) const FULLSCREEN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M8 3H5a2 2 0 0 0-2 2v3M21 8V5a2 2 0 0 0-2-2h-3M3 16v3a2 2 0 0 0 2 2h3M16 21h3a2 2 0 0 0 2-2v-3"/></svg>"#;

pub(super) const FULLSCREEN_EXIT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M8 3v3a2 2 0 0 1-2 2H3M21 8h-3a2 2 0 0 1-2-2V3M3 16h3a2 2 0 0 1 2 2v3M16 21v-3a2 2 0 0 1 2-2h3"/></svg>"#;

/// Status: what the view shows (the proxy, the OSV or full res, a full-res
/// frame on its way) and how it decodes (on the GPU, or in software).
pub(super) const SHOWING_PROXY: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M7 3v18M17 3v18M3 7.5h4M3 12h18M3 16.5h4M17 7.5h4M17 16.5h4"/></svg>"#;

pub(super) const SHOWING_FULL: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-3.1-3.1a2 2 0 0 0-2.8 0L6 21"/></svg>"#;

pub(super) const SHOWING_LOADING: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M5 22h14M5 2h14M17 22v-4.2a2 2 0 0 0-.6-1.4L12 12l-4.4 4.4a2 2 0 0 0-.6 1.4V22M7 2v4.2a2 2 0 0 0 .6 1.4L12 12l4.4-4.4a2 2 0 0 0 .6-1.4V2"/></svg>"#;

pub(super) const DECODE_GPU: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M13 2 3 14h9l-1 8 10-12h-9z"/></svg>"#;

pub(super) const DECODE_CPU: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="4" y="4" width="16" height="16" rx="2"/><rect x="9" y="9" width="6" height="6" rx="1"/><path d="M9 1v3M15 1v3M9 20v3M15 20v3M20 9h3M20 14h3M1 9h3M1 14h3"/></svg>"#;

/// Speaker icons for the mute button, drawn in the button's text colour.
pub(super) const SPEAKER: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11 5 6 9H2v6h4l5 4z" fill="white"/><path d="M15.5 8.5a5 5 0 0 1 0 7"/><path d="M19 5a10 10 0 0 1 0 14"/></svg>"#;

pub(super) const SPEAKER_MUTED: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11 5 6 9H2v6h4l5 4z" fill="white"/><path d="m22 9-6 6"/><path d="m16 9 6 6"/></svg>"#;

/// Omnix's own mark (`packaging/omnix.svg`, the app's icon too): a
/// sphere over a timeline, its playhead through the lanes.
pub(super) const LOGO: &[u8] = include_bytes!("../../packaging/omnix.svg");

/// Who made it, and how: Omnix made with Claude, and, apart, rOsmo360,
/// whose stitching and player it grew from (user, 2026-11-05, gap 79).
pub(super) fn credit<'a>() -> Element<'a, Message> {
    let line = concat!("Omnix ", env!("CARGO_PKG_VERSION"), " · made by Dmytro Halushko with Claude · 2026  ·  built on rOsmo360 by Vibing with Ravelund");
    container(text(line).size(12).style(text::secondary)).width(Length::Fill).align_x(Alignment::Center).padding([2, 10]).into()
}

/// A hint on hover, on a box of its own so it reads over the picture.
pub(super) fn tip<'a>(content: impl Into<Element<'a, Message>>, hint: iced::widget::Text<'a>, position: tooltip::Position) -> Element<'a, Message> {
    tooltip(content, container(hint).padding([4, 8]).style(container::rounded_box), position).into()
}

pub(super) fn icon_button<'a>(icon: &'static [u8], on_press: Option<Message>) -> Element<'a, Message> {
    button(svg_icon(icon)).padding(5).on_press_maybe(on_press).into()
}

/// The bars' groups' own tints (gap 66: all buttons were the same blue,
/// and the user wanted to find a group by its colour). Muted, so the white
/// icons read and the picture keeps the eye; a group has its tint wherever
/// it appears. Never colour alone: the icons and the groups' names say it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Tint {
    /// The app, the project file, the export, the window.
    Global,
    /// Playing and moving about: play, the zoom, snapping.
    Move,
    /// Adding at the playhead.
    Add,
    /// Cutting: split, delete, crossfade.
    Cut,
    History,
    /// The look: effects.
    Look,
    /// Framing: the view's keyframes.
    Frame,
    Sound,
}

impl Tint {
    pub(super) fn colour(self) -> iced::Color {
        let c = iced::Color::from_rgb8;
        match self {
            Tint::Global => c(0x4a, 0x50, 0x5a),
            Tint::Move => c(0x2f, 0x5f, 0x9e),
            Tint::Add => c(0x3a, 0x7a, 0x50),
            Tint::Cut => c(0x9a, 0x60, 0x26),
            Tint::History => c(0x5e, 0x5e, 0x5e),
            Tint::Look => c(0x6e, 0x55, 0x9e),
            Tint::Frame => c(0x8a, 0x46, 0x78),
            Tint::Sound => c(0x26, 0x78, 0x7e),
        }
    }
}

/// An icon button in its group's tint: lighter hovered, darker pressed,
/// faint when it can't be pressed.
pub(super) fn tint_button<'a>(tint: Tint, icon: &'static [u8], on_press: Option<Message>) -> Element<'a, Message> {
    let base = tint.colour();
    let mix = move |to: f32, k: f32| iced::Color { r: base.r + (to - base.r) * k, g: base.g + (to - base.g) * k, b: base.b + (to - base.b) * k, a: 1.0 };
    button(svg_icon(icon))
        .padding(5)
        .style(move |_: &iced::Theme, status| {
            let background = match status {
                button::Status::Hovered => mix(1.0, 0.15),
                button::Status::Pressed => mix(0.0, 0.2),
                button::Status::Disabled => base.scale_alpha(0.35),
                button::Status::Active => base,
            };
            button::Style { background: Some(background.into()), text_color: iced::Color::WHITE, border: iced::Border { radius: 3.0.into(), ..Default::default() }, ..button::Style::default() }
        })
        .on_press_maybe(on_press)
        .into()
}

/// An icon as the buttons show it.
pub(super) fn svg_icon(icon: &'static [u8]) -> svg::Svg<'static> {
    svg(svg::Handle::from_memory(icon)).width(20).height(20)
}

/// A mix level for its slider: the bottom of the range is silence.
pub(super) fn db_label(db: f64) -> String {
    if db <= crate::project::OFF_DB { "off".to_owned() } else { format!("{db:+.1} dB") }
}

/// A status icon, dimmed towards the background so it stays out of the
/// way, with what it means on hover.
pub(super) fn status_icon<'a>(icon: &'static [u8], hint: String) -> Element<'a, Message> {
    let dim = svg(svg::Handle::from_memory(icon)).width(18).height(18).style(|theme: &Theme, _| svg::Style {
        color: Some(iced::Color { a: 0.45, ..theme.palette().text }),
    });
    tip(dim, text(hint), tooltip::Position::Bottom)
}

/// An icon with its words, for the start screen, which has the room.
pub(super) fn labelled_button<'a>(icon: &'static [u8], label: &'a str, on_press: Option<Message>) -> Element<'a, Message> {
    button(row![svg(svg::Handle::from_memory(icon)).width(20).height(20), text(label)].spacing(8).align_y(Alignment::Center))
        .padding([6, 12])
        .on_press_maybe(on_press)
        .into()
}

/// A thin upright line between groups of buttons in a bar.
pub(super) fn separator<'a>() -> Element<'a, Message> {
    container(rule::vertical(1)).height(22).into()
}

pub(super) fn labelled<'a>(label: &'a str, widget: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    row![text(label), widget.into()].spacing(6).align_y(Alignment::Center).into()
}

/// Keyboard: the shortcuts' list.
pub(super) const KEYBOARD: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="5" width="20" height="14" rx="2"/><path d="M6 9h.01M10 9h.01M14 9h.01M18 9h.01M6 13h.01M18 13h.01M10 13h4M7 16h10"/></svg>"#;
