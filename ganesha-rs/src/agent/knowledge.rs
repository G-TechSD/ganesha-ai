//! UI Knowledge Base
//!
//! Repository of information about common UI patterns, applications,
//! and navigation strategies. Helps the agent understand how to interact
//! with different applications and desktop environments.

use std::collections::HashMap;

/// Knowledge about a specific application
#[derive(Debug, Clone)]
pub struct AppKnowledge {
    pub name: String,
    pub window_titles: Vec<String>,
    pub process_names: Vec<String>,
    pub launch_methods: Vec<LaunchMethod>,
    pub key_elements: Vec<UIElement>,
    pub common_dialogs: Vec<DialogPattern>,
    pub keyboard_shortcuts: HashMap<String, String>,
    pub close_method: CloseMethod,
}

/// How to launch an application
#[derive(Debug, Clone)]
pub enum LaunchMethod {
    /// Search in Activities/Start menu
    ActivitiesSearch { query: String },
    /// Direct command
    Command { cmd: String, args: Vec<String> },
    /// Desktop icon at approximate position
    DesktopIcon { name: String },
    /// Dock/taskbar icon at position (from left)
    DockIcon { position: u32 },
}

/// Known UI element in an application
#[derive(Debug, Clone)]
pub struct UIElement {
    pub name: String,
    pub description: String,
    pub typical_location: ElementLocation,
    pub find_strategy: FindStrategy,
}

/// Where to typically find an element
#[derive(Debug, Clone)]
pub enum ElementLocation {
    /// Fixed position (percentage of screen)
    Fixed { x_pct: f32, y_pct: f32 },
    /// Relative to window edge
    WindowRelative { edge: Edge, offset_x: i32, offset_y: i32 },
    /// In a menu bar
    MenuBar { menu_name: String, item_name: String },
    /// By visual search
    Visual { description: String },
}

#[derive(Debug, Clone)]
pub enum Edge {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Top,
    Bottom,
    Left,
    Right,
}

/// Strategy for finding an element
#[derive(Debug, Clone)]
pub enum FindStrategy {
    /// Ask vision model to find by description
    VisionSearch { description: String },
    /// Use OCR to find text
    TextMatch { text: String, case_sensitive: bool },
    /// Known fixed coordinates (fallback)
    FixedCoords { x: i32, y: i32 },
    /// Keyboard shortcut instead of clicking
    KeyboardShortcut { shortcut: String },
}

/// Common dialog patterns
#[derive(Debug, Clone)]
pub struct DialogPattern {
    pub name: String,
    pub triggers: Vec<String>,  // What causes this dialog
    pub detection: String,      // How to detect it's open
    pub buttons: Vec<DialogButton>,
    pub default_action: String, // Usually "Cancel" or "OK"
}

#[derive(Debug, Clone)]
pub struct DialogButton {
    pub label: String,
    pub action: String,  // What it does
    pub shortcut: Option<String>,  // Keyboard shortcut if any
    pub is_default: bool,
}

/// How to close an application
#[derive(Debug, Clone)]
pub enum CloseMethod {
    /// Window close button
    CloseButton,
    /// Alt+F4
    AltF4,
    /// Specific menu item
    MenuItem { menu: String, item: String },
    /// Keyboard shortcut
    Shortcut { keys: String },
    /// wmctrl command
    WmCtrl,
}

/// Desktop environment knowledge
#[derive(Debug, Clone)]
pub struct DesktopKnowledge {
    pub name: String,  // GNOME, KDE, etc.
    pub activities_trigger: ActivitiesTrigger,
    pub taskbar_position: TaskbarPosition,
    pub window_controls: WindowControls,
    pub common_shortcuts: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub enum ActivitiesTrigger {
    /// Hot corner at position
    HotCorner { corner: Edge },
    /// Super/Windows key
    SuperKey,
    /// Click on specific area
    ClickArea { x: i32, y: i32, width: u32, height: u32 },
    /// None (no activities view)
    None,
}

#[derive(Debug, Clone)]
pub enum TaskbarPosition {
    Top,
    Bottom,
    Left,
    Right,
    Hidden,
}

#[derive(Debug, Clone)]
pub struct WindowControls {
    pub position: Edge,  // TopLeft, TopRight
    pub order: Vec<String>,  // ["minimize", "maximize", "close"]
    pub close_offset: (i32, i32),  // Offset from corner
}

/// The main knowledge base
pub struct UIKnowledgeBase {
    pub desktop: DesktopKnowledge,
    pub apps: HashMap<String, AppKnowledge>,
    pub universal_shortcuts: HashMap<String, String>,
    pub dialog_handlers: Vec<DialogPattern>,
}

impl UIKnowledgeBase {
    /// Create knowledge base for GNOME desktop
    pub fn gnome() -> Self {
        let mut apps = HashMap::new();
        let mut universal_shortcuts = HashMap::new();

        // Universal shortcuts
        universal_shortcuts.insert("copy".into(), "ctrl+c".into());
        universal_shortcuts.insert("paste".into(), "ctrl+v".into());
        universal_shortcuts.insert("cut".into(), "ctrl+x".into());
        universal_shortcuts.insert("undo".into(), "ctrl+z".into());
        universal_shortcuts.insert("redo".into(), "ctrl+shift+z".into());
        universal_shortcuts.insert("save".into(), "ctrl+s".into());
        universal_shortcuts.insert("save_as".into(), "ctrl+shift+s".into());
        universal_shortcuts.insert("open".into(), "ctrl+o".into());
        universal_shortcuts.insert("new".into(), "ctrl+n".into());
        universal_shortcuts.insert("close_window".into(), "alt+F4".into());
        universal_shortcuts.insert("close_tab".into(), "ctrl+w".into());
        universal_shortcuts.insert("find".into(), "ctrl+f".into());
        universal_shortcuts.insert("select_all".into(), "ctrl+a".into());
        universal_shortcuts.insert("print".into(), "ctrl+p".into());

        // Firefox knowledge
        apps.insert("firefox".into(), AppKnowledge {
            name: "Firefox".into(),
            window_titles: vec!["Firefox".into(), "Mozilla Firefox".into()],
            process_names: vec!["firefox".into(), "firefox-esr".into()],
            launch_methods: vec![
                LaunchMethod::ActivitiesSearch { query: "firefox".into() },
                LaunchMethod::Command { cmd: "firefox".into(), args: vec![] },
            ],
            key_elements: vec![
                UIElement {
                    name: "address_bar".into(),
                    description: "URL/address bar at top of browser".into(),
                    typical_location: ElementLocation::WindowRelative {
                        edge: Edge::Top,
                        offset_x: 400,
                        offset_y: 40,
                    },
                    find_strategy: FindStrategy::KeyboardShortcut { shortcut: "ctrl+l".into() },
                },
                UIElement {
                    name: "new_tab".into(),
                    description: "New tab button (+ icon)".into(),
                    typical_location: ElementLocation::Visual {
                        description: "plus icon next to last tab".into(),
                    },
                    find_strategy: FindStrategy::KeyboardShortcut { shortcut: "ctrl+t".into() },
                },
                UIElement {
                    name: "back_button".into(),
                    description: "Back navigation button".into(),
                    typical_location: ElementLocation::WindowRelative {
                        edge: Edge::TopLeft,
                        offset_x: 50,
                        offset_y: 40,
                    },
                    find_strategy: FindStrategy::KeyboardShortcut { shortcut: "alt+Left".into() },
                },
            ],
            common_dialogs: vec![
                DialogPattern {
                    name: "restore_session".into(),
                    triggers: vec!["crash".into(), "restart".into()],
                    detection: "restore previous session".into(),
                    buttons: vec![
                        DialogButton {
                            label: "Restore".into(),
                            action: "restore_tabs".into(),
                            shortcut: Some("Enter".into()),
                            is_default: true,
                        },
                        DialogButton {
                            label: "Start New Session".into(),
                            action: "new_session".into(),
                            shortcut: None,
                            is_default: false,
                        },
                    ],
                    default_action: "Escape".into(),
                },
            ],
            keyboard_shortcuts: {
                let mut ks = HashMap::new();
                ks.insert("address_bar".into(), "ctrl+l".into());
                ks.insert("new_tab".into(), "ctrl+t".into());
                ks.insert("close_tab".into(), "ctrl+w".into());
                ks.insert("reload".into(), "ctrl+r".into());
                ks.insert("back".into(), "alt+Left".into());
                ks.insert("forward".into(), "alt+Right".into());
                ks.insert("find".into(), "ctrl+f".into());
                ks.insert("bookmarks".into(), "ctrl+b".into());
                ks.insert("history".into(), "ctrl+h".into());
                ks.insert("developer_tools".into(), "F12".into());
                ks
            },
            close_method: CloseMethod::WmCtrl,
        });

        // LibreOffice Writer knowledge
        apps.insert("writer".into(), AppKnowledge {
            name: "LibreOffice Writer".into(),
            window_titles: vec!["LibreOffice Writer".into(), "Writer".into()],
            process_names: vec!["soffice".into(), "soffice.bin".into()],
            launch_methods: vec![
                LaunchMethod::ActivitiesSearch { query: "writer".into() },
                LaunchMethod::Command { cmd: "libreoffice".into(), args: vec!["--writer".into()] },
            ],
            key_elements: vec![
                UIElement {
                    name: "document_area".into(),
                    description: "Main document editing area".into(),
                    typical_location: ElementLocation::Fixed { x_pct: 0.5, y_pct: 0.5 },
                    find_strategy: FindStrategy::VisionSearch {
                        description: "white document area in center".into(),
                    },
                },
                UIElement {
                    name: "menu_bar".into(),
                    description: "Menu bar (File, Edit, View...)".into(),
                    typical_location: ElementLocation::WindowRelative {
                        edge: Edge::Top,
                        offset_x: 100,
                        offset_y: 30,
                    },
                    find_strategy: FindStrategy::TextMatch {
                        text: "File".into(),
                        case_sensitive: true,
                    },
                },
            ],
            common_dialogs: vec![
                DialogPattern {
                    name: "save_changes".into(),
                    triggers: vec!["close".into(), "exit".into()],
                    detection: "save changes".into(),
                    buttons: vec![
                        DialogButton {
                            label: "Save".into(),
                            action: "save_and_close".into(),
                            shortcut: Some("s".into()),
                            is_default: false,
                        },
                        DialogButton {
                            label: "Don't Save".into(),
                            action: "discard_and_close".into(),
                            shortcut: Some("d".into()),
                            is_default: false,
                        },
                        DialogButton {
                            label: "Cancel".into(),
                            action: "cancel_close".into(),
                            shortcut: Some("Escape".into()),
                            is_default: true,
                        },
                    ],
                    default_action: "d".into(),  // Don't Save
                },
                DialogPattern {
                    name: "format_warning".into(),
                    triggers: vec!["save".into()],
                    detection: "keep odf format".into(),
                    buttons: vec![
                        DialogButton {
                            label: "Use ODF Format".into(),
                            action: "save_odf".into(),
                            shortcut: Some("Enter".into()),
                            is_default: true,
                        },
                        DialogButton {
                            label: "Use Other Format".into(),
                            action: "save_other".into(),
                            shortcut: None,
                            is_default: false,
                        },
                    ],
                    default_action: "Enter".into(),
                },
            ],
            keyboard_shortcuts: {
                let mut ks = HashMap::new();
                ks.insert("save".into(), "ctrl+s".into());
                ks.insert("save_as".into(), "ctrl+shift+s".into());
                ks.insert("bold".into(), "ctrl+b".into());
                ks.insert("italic".into(), "ctrl+i".into());
                ks.insert("underline".into(), "ctrl+u".into());
                ks.insert("find_replace".into(), "ctrl+h".into());
                ks.insert("go_to_start".into(), "ctrl+Home".into());
                ks.insert("go_to_end".into(), "ctrl+End".into());
                ks.insert("select_line".into(), "shift+End".into());
                ks
            },
            close_method: CloseMethod::WmCtrl,
        });

        // Files (Nautilus) knowledge
        apps.insert("files".into(), AppKnowledge {
            name: "Files".into(),
            window_titles: vec!["Files".into(), "Nautilus".into()],
            process_names: vec!["nautilus".into()],
            launch_methods: vec![
                LaunchMethod::ActivitiesSearch { query: "files".into() },
                LaunchMethod::Command { cmd: "nautilus".into(), args: vec![] },
            ],
            key_elements: vec![
                UIElement {
                    name: "path_bar".into(),
                    description: "Path/location bar".into(),
                    typical_location: ElementLocation::WindowRelative {
                        edge: Edge::Top,
                        offset_x: 300,
                        offset_y: 50,
                    },
                    find_strategy: FindStrategy::KeyboardShortcut { shortcut: "ctrl+l".into() },
                },
                UIElement {
                    name: "search".into(),
                    description: "Search button/box".into(),
                    typical_location: ElementLocation::WindowRelative {
                        edge: Edge::Top,
                        offset_x: -100,
                        offset_y: 30,
                    },
                    find_strategy: FindStrategy::KeyboardShortcut { shortcut: "ctrl+f".into() },
                },
            ],
            common_dialogs: vec![],
            keyboard_shortcuts: {
                let mut ks = HashMap::new();
                ks.insert("path_bar".into(), "ctrl+l".into());
                ks.insert("search".into(), "ctrl+f".into());
                ks.insert("new_folder".into(), "ctrl+shift+n".into());
                ks.insert("delete".into(), "Delete".into());
                ks.insert("rename".into(), "F2".into());
                ks.insert("properties".into(), "alt+Return".into());
                ks
            },
            close_method: CloseMethod::WmCtrl,
        });

        // Terminal knowledge
        apps.insert("terminal".into(), AppKnowledge {
            name: "Terminal".into(),
            window_titles: vec!["Terminal".into(), "GNOME Terminal".into()],
            process_names: vec!["gnome-terminal".into(), "gnome-terminal-server".into()],
            launch_methods: vec![
                LaunchMethod::ActivitiesSearch { query: "terminal".into() },
                LaunchMethod::Command { cmd: "gnome-terminal".into(), args: vec![] },
            ],
            key_elements: vec![],
            common_dialogs: vec![
                DialogPattern {
                    name: "close_running".into(),
                    triggers: vec!["close".into()],
                    detection: "process is still running".into(),
                    buttons: vec![
                        DialogButton {
                            label: "Close Terminal".into(),
                            action: "force_close".into(),
                            shortcut: Some("Enter".into()),
                            is_default: true,
                        },
                    ],
                    default_action: "Enter".into(),
                },
            ],
            keyboard_shortcuts: {
                let mut ks = HashMap::new();
                ks.insert("copy".into(), "ctrl+shift+c".into());
                ks.insert("paste".into(), "ctrl+shift+v".into());
                ks.insert("new_tab".into(), "ctrl+shift+t".into());
                ks.insert("close_tab".into(), "ctrl+shift+w".into());
                ks
            },
            close_method: CloseMethod::WmCtrl,
        });

        // Common dialog handlers
        let dialog_handlers = vec![
            DialogPattern {
                name: "generic_save".into(),
                triggers: vec!["close".into(), "quit".into()],
                detection: "save changes".into(),
                buttons: vec![
                    DialogButton {
                        label: "Save".into(),
                        action: "save".into(),
                        shortcut: Some("s".into()),
                        is_default: false,
                    },
                    DialogButton {
                        label: "Don't Save".into(),
                        action: "discard".into(),
                        shortcut: Some("d".into()),
                        is_default: false,
                    },
                    DialogButton {
                        label: "Cancel".into(),
                        action: "cancel".into(),
                        shortcut: Some("Escape".into()),
                        is_default: true,
                    },
                ],
                default_action: "Escape".into(),
            },
            DialogPattern {
                name: "generic_confirm".into(),
                triggers: vec!["delete".into(), "overwrite".into()],
                detection: "are you sure".into(),
                buttons: vec![
                    DialogButton {
                        label: "Yes".into(),
                        action: "confirm".into(),
                        shortcut: Some("y".into()),
                        is_default: false,
                    },
                    DialogButton {
                        label: "No".into(),
                        action: "cancel".into(),
                        shortcut: Some("n".into()),
                        is_default: true,
                    },
                ],
                default_action: "Escape".into(),
            },
        ];

        Self {
            desktop: DesktopKnowledge {
                name: "GNOME".into(),
                activities_trigger: ActivitiesTrigger::HotCorner { corner: Edge::TopLeft },
                taskbar_position: TaskbarPosition::Top,
                window_controls: WindowControls {
                    position: Edge::TopRight,
                    order: vec!["minimize".into(), "maximize".into(), "close".into()],
                    close_offset: (-20, 15),
                },
                common_shortcuts: {
                    let mut ks = HashMap::new();
                    ks.insert("activities".into(), "super".into());
                    ks.insert("app_menu".into(), "super+a".into());
                    ks.insert("screenshot".into(), "Print".into());
                    ks.insert("screenshot_window".into(), "alt+Print".into());
                    ks.insert("screenshot_area".into(), "shift+Print".into());
                    ks.insert("lock_screen".into(), "super+l".into());
                    ks.insert("switch_window".into(), "alt+Tab".into());
                    ks.insert("close_window".into(), "alt+F4".into());
                    ks.insert("maximize".into(), "super+Up".into());
                    ks.insert("minimize".into(), "super+h".into());
                    ks
                },
            },
            apps,
            universal_shortcuts,
            dialog_handlers,
        }
    }

    /// Get app knowledge by name
    pub fn get_app(&self, name: &str) -> Option<&AppKnowledge> {
        let name_lower = name.to_lowercase();
        self.apps.get(&name_lower)
            .or_else(|| {
                self.apps.values().find(|app| {
                    app.name.to_lowercase().contains(&name_lower) ||
                    app.window_titles.iter().any(|t| t.to_lowercase().contains(&name_lower))
                })
            })
    }

    /// Get shortcut for an action
    pub fn get_shortcut(&self, app: Option<&str>, action: &str) -> Option<String> {
        // First check app-specific shortcuts
        if let Some(app_name) = app {
            if let Some(app_knowledge) = self.get_app(app_name) {
                if let Some(shortcut) = app_knowledge.keyboard_shortcuts.get(action) {
                    return Some(shortcut.clone());
                }
            }
        }

        // Fall back to universal shortcuts
        self.universal_shortcuts.get(action).cloned()
    }

    /// Find dialog handler for detected dialog
    pub fn find_dialog_handler(&self, dialog_text: &str) -> Option<&DialogPattern> {
        let text_lower = dialog_text.to_lowercase();

        // Check app-specific dialogs first
        for app in self.apps.values() {
            for dialog in &app.common_dialogs {
                if text_lower.contains(&dialog.detection.to_lowercase()) {
                    return Some(dialog);
                }
            }
        }

        // Check generic handlers
        for dialog in &self.dialog_handlers {
            if text_lower.contains(&dialog.detection.to_lowercase()) {
                return Some(dialog);
            }
        }

        None
    }

    /// Get the best launch method for an app
    pub fn get_launch_method(&self, app_name: &str) -> Option<&LaunchMethod> {
        self.get_app(app_name)
            .and_then(|app| app.launch_methods.first())
    }

    /// Get close method for an app
    pub fn get_close_method(&self, app_name: &str) -> CloseMethod {
        self.get_app(app_name)
            .map(|app| app.close_method.clone())
            .unwrap_or(CloseMethod::WmCtrl)
    }

    /// Get Activities trigger for current desktop
    pub fn activities_shortcut(&self) -> String {
        match &self.desktop.activities_trigger {
            ActivitiesTrigger::SuperKey => "super".into(),
            ActivitiesTrigger::HotCorner { .. } => "super".into(),  // Super key also works
            ActivitiesTrigger::ClickArea { .. } => "super".into(),
            ActivitiesTrigger::None => "".into(),
        }
    }
}

impl Default for UIKnowledgeBase {
    fn default() -> Self {
        Self::gnome()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_knowledge_base() {
        let kb = UIKnowledgeBase::gnome();

        // Test app lookup
        assert!(kb.get_app("firefox").is_some());
        assert!(kb.get_app("Firefox").is_some());
        assert!(kb.get_app("writer").is_some());

        // Test shortcut lookup
        assert_eq!(kb.get_shortcut(Some("firefox"), "address_bar"), Some("ctrl+l".into()));
        assert_eq!(kb.get_shortcut(None, "copy"), Some("ctrl+c".into()));
    }
}
