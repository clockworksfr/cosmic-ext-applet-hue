// SPDX-License-Identifier: MIT

use crate::config::Config;
use crate::fl;
use cosmic::cctk::wayland_protocols::xdg::shell::client::xdg_positioner::Anchor;
use cosmic::cctk::wayland_protocols::xdg::shell::client::xdg_positioner::Gravity;
use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::iced::{Alignment, Length, Rectangle, Subscription};
use cosmic::iced::{Limits, window::Id};
use cosmic::iced_winit::commands::popup::{destroy_popup, get_popup};
use cosmic::widget::icon;
use cosmic::widget::color_picker::ColorPickerUpdate;
use cosmic::widget::rectangle_tracker::RectangleUpdate;
use cosmic::widget::{self, RectangleTracker, rectangle_tracker};
use cosmic::{Action, Task};
use cosmic::{iced_core, prelude::*};
use huelib;
use huelib::response::{Modified, Response};
use std::collections::HashMap;
use std::net::IpAddr;
use std::net::Ipv4Addr;

/// The application model stores app-specific state used to describe its interface and
/// drive its logic.
pub struct AppModel {
    /// Application state which is managed by the COSMIC runtime.
    core: cosmic::Core,
    /// The popup id.
    popup: Option<Id>,
    /// Configuration data that persists between application runs.
    config: Config,
    /// The app is scanning for bridges.
    is_scanning: bool,
    /// The last discovery result.
    last_discovery: Option<Result<IpAddr, String>>,
    /// The lights.
    lights: Vec<LightVm>,
    /// The groups.
    groups: Vec<GroupVm>,
    /// The scenes.
    scenes: Vec<SceneVm>,
    /// Lights menu expanded.
    lights_menu_expanded: bool,
    /// Groups menu expanded.
    groups_menu_expanded: bool,
    /// Scenes menu expanded.
    scenes_menu_expanded: bool,
    /// The color picker popup id.
    color_picker_popup: Option<Id>,
    /// The more menu popup id.
    more_menu_popup: Option<Id>,
    /// Active color picker item id.
    active_color_picker_item: Option<(String, String)>,
    /// Last active color picker item id.
    last_active_color_picker_item: Option<String>,
    /// The color picker model.
    color_picker_model: widget::ColorPickerModel,
    /// A tracker for positioning the color picker popup.
    color_button_tracker: Option<RectangleTracker<u32>>,
    /// A map to keep the rectangles of the color buttons.
    color_button_rectangles: HashMap<u32, Rectangle>,
    /// Pending light brightness changes (light_id, brightness, counter)
    pending_light_brightness: HashMap<String, (f32, u64)>,
    /// Pending group brightness changes (group_id, brightness, counter)
    pending_group_brightness: HashMap<String, (f32, u64)>,
    /// Pending light color changes (light_id, (hue, saturation, value), counter)
    pending_light_color: HashMap<String, ((u16, u8, u8), u64)>,
    /// Pending group color changes (group_id, (hue, saturation, value), counter)
    pending_group_color: HashMap<String, ((u16, u8, u8), u64)>,
    /// Counter for debounce operations
    debounce_counter: u64,
}

pub struct LightVm {
    id: String,
    name: String,
    on: Option<bool>,
    brightness: Option<u8>,
    color: Option<(f32, f32, f32)>,
}

pub struct GroupVm {
    id: String,
    name: String,
    on: Option<bool>,
    brightness: Option<u8>,
    color: Option<(f32, f32, f32)>,
    lights: Vec<String>,
}

pub struct SceneVm {
    id: String,
    name: String,
    group: String,
}

/// Messages emitted by the application and its widgets.
#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    DiscoverBridge,
    BridgeDiscoveryFinished(Result<IpAddr, String>),
    PairBridge,
    PairBridgeFinished(Result<String, String>),
    LoadGroups,
    GroupsLoaded(Result<Vec<huelib::resource::Group>, String>),
    LoadScenes,
    ScenesLoaded(Result<Vec<huelib::resource::Scene>, String>),
    LoadLights,
    LightsLoaded(Result<Vec<huelib::resource::Light>, String>),
    ToggleLight(String, bool),
    ToggleGroup(String, bool),
    ActivateScene(String),
    ResponsesModified(Result<Vec<Response<Modified>>, String>),
    ToggleLightsMenu,
    ToggleGroupsMenu,
    ToggleScenesMenu,
    SetLightBrightness(String, f32),
    SetGroupBrightness(String, f32),
    ToggleColorPicker((String, String)),
    TryToggleColorPicker,
    SetLightColor(widget::color_picker::ColorPickerUpdate),
    SetGroupColor(widget::color_picker::ColorPickerUpdate),
    RectanglesUpdated(RectangleUpdate<u32>),
    ApplyLightBrightness(String, u64),
    ApplyGroupBrightness(String, u64),
    ApplyLightColor(String, u64),
    ApplyGroupColor(String, u64),
    SceneActivated(Result<Vec<Response<Modified>>, String>),
    ToggleMoreMenu,
    UnpairBridge,
}

/// Create a COSMIC application from the app model
impl cosmic::Application for AppModel {
    /// The async executor that will be used to run your application's commands.
    type Executor = cosmic::executor::Default;

    /// Data that your application receives to its init method.
    type Flags = ();

    /// Messages which the application and its widgets will emit.
    type Message = Message;

    /// Unique identifier in RDNN (reverse domain name notation) format.
    const APP_ID: &'static str = "com.clockworksfr.cosmichue";

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    /// Initializes the application with any given flags and startup commands.
    fn init(
        core: cosmic::Core,
        _flags: Self::Flags,
    ) -> (Self, Task<cosmic::Action<Self::Message>>) {
        // Construct the app model with the runtime's core.
        let app = AppModel {
            core,
            popup: None,
            is_scanning: false,
            last_discovery: None,
            lights: Vec::new(),
            groups: Vec::new(),
            scenes: Vec::new(),
            lights_menu_expanded: false,
            groups_menu_expanded: false,
            scenes_menu_expanded: false,
            config: cosmic_config::Config::new(Self::APP_ID, Config::VERSION)
                .map(|context| match Config::get_entry(&context) {
                    Ok(config) => config,
                    Err((_errors, config)) => {
                        // for why in errors {
                        //     tracing::error!(%why, "error loading app config");
                        // }

                        config
                    }
                })
                .unwrap_or_default(),
            color_picker_model: widget::ColorPickerModel::new("hex", "rgb", None, None),
            active_color_picker_item: None,
            last_active_color_picker_item: None,
            color_picker_popup: None,
            more_menu_popup: None,
            color_button_tracker: None,
            color_button_rectangles: HashMap::new(),
            pending_light_brightness: HashMap::new(),
            pending_group_brightness: HashMap::new(),
            pending_light_color: HashMap::new(),
            pending_group_color: HashMap::new(),
            debounce_counter: 0,
        };

        (app, Task::none())
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn view(&self) -> Element<'_, Self::Message> {
        // Load the custom lightbulb icon
        const LIGHTBULB_ICON: &[u8] = include_bytes!("../resources/icon.svg");
        let icon_handle = icon::from_svg_bytes(LIGHTBULB_ICON);
        
        self.core
            .applet
            .icon_button_from_handle(icon_handle)
            .on_press(Message::TogglePopup)
            .into()
    }

    fn view_window(&self, id: Id) -> Element<'_, Self::Message> {        
        if Some(id) == self.more_menu_popup {
            let container = widget::container(
                        widget::column::with_children(vec![
                            widget::flex_row(
                                vec![
                                    widget::text(fl!("bridge-ip")).into(),
                                    widget::horizontal_space().into(),
                                    widget::text(self.config.get_bridge_ip().unwrap_or(&IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))).to_string()).into(),
                                ]
                            ).padding(10).into(),
                            widget::divider::horizontal::default().into(),
                            widget::flex_row(
                                vec![
                                    widget::button::destructive(fl!("unpair-bridge")).on_press(Message::UnpairBridge).into(),
                                ]
                            ).padding(10).into(),
                        ])
                        .spacing(10)
                ).padding(10).style(
                    |theme| widget::container::Style {
                        border: cosmic::iced::Border {
                            color: theme.cosmic().accent.base.into(),
                            width: 2.0,
                            radius: 8.0.into(),
                        },
                        ..Default::default()
                    },
                );
            
            self.core
                .applet
                .popup_container(container)
                .min_width(120.0)
                .max_width(360.0)
                .limits(Limits::NONE.min_width(120.0).max_width(360.0))
                .into()
        } else if Some(id) == self.color_picker_popup {

            let message = match self.active_color_picker_item.as_ref() {
                Some((_, item_type)) if item_type == "light" => Message::SetLightColor,
                Some((_, item_type)) if item_type == "group" => Message::SetGroupColor,
                _ => return widget::text(fl!("no-color-picker-item-selected")).into(),
            };

            let color_picker_content = self.color_picker_model.builder(message).build(
                "Recent colors",
                "Copy to clipboard",
                "Copied to clipboard",
            );

            let container =
                widget::container(color_picker_content).padding(10).style(
                    |theme| widget::container::Style {
                        border: cosmic::iced::Border {
                            color: theme.cosmic().accent.base.into(),
                            width: 2.0,
                            radius: 8.0.into(),
                        },
                        ..Default::default()
                    },
                );

            self.core
                .applet
                .popup_container(container)
                .min_width(120.0)
                .max_width(360.0)
                .limits(Limits::NONE.min_width(120.0).max_width(360.0))
                .into()
        } else {
            let mut content_list = widget::list_column().add(widget::text(fl!("app-title")).align_y(Alignment::Center).height(30.0));

            if self.config.get_username().is_none() {
                let button = if self.is_scanning {
                    widget::button::text(fl!("configure"))
                } else {
                    widget::button::text(fl!("configure")).on_press(Message::DiscoverBridge)
                };

                let discovery_text = if self.is_scanning {
                    fl!("searching-for-bridges").to_string()
                } else {
                    match &self.last_discovery {
                        Some(Ok(bridge_ip)) => fl!("bridge-found", bridge_ip = bridge_ip.to_string()),
                        Some(Err(error)) => fl!("error", error = error.clone()),
                        None => fl!("no-bridge-configured").to_string(),
                    }
                };

                let discovery_text = widget::text(discovery_text);
                content_list = content_list.add(discovery_text);
                content_list = content_list.add(button);

                if let Some(bridge_ip) = self.config.get_bridge_ip() {
                    content_list = content_list.add(widget::text(fl!("bridge-found-description")));
                    content_list = content_list.add(
                        widget::flex_row(vec![
                            widget::text(fl!("bridge", bridge_ip = bridge_ip.to_string())).into(),
                            widget::horizontal_space().into(),
                            widget::button::text(fl!("pair-bridge")).on_press(Message::PairBridge).into(),
                        ]));
                }
            } else {
                content_list = widget::list_column().add(
                    widget::flex_row(vec![
                        widget::text(fl!("app-title")).align_y(Alignment::Center).height(30.0).into(),
                        widget::horizontal_space().into(),
                        widget::button::icon(widget::icon::from_name("view-more-symbolic")).on_press(Message::ToggleMoreMenu).into(),
                    ])
                ).into();
                
                // Load data on popup opening
                if self.popup.is_some() {
                    let _ = Task::batch([
                        Task::perform(async {}, |_| Action::App(Message::LoadLights)),
                        Task::perform(async {}, |_| Action::App(Message::LoadGroups)),
                        Task::perform(async {}, |_| Action::App(Message::LoadScenes)),
                    ]);
                }

                // Build the lights list
                content_list = content_list.add(self.build_lights_section());

                // Build the groups list
                content_list = content_list.add(self.build_groups_section());

                // Build the scenes list
                content_list = content_list.add(self.build_scenes_section());
            }
            content_list = content_list.into();

            let main_container = widget::container(content_list).padding(10);
    
            self.core
                .applet
                .popup_container(main_container)
                .min_width(200.0)
                .max_width(200.0)
                .limits(Limits::NONE.min_width(500.0).max_width(500.0))
                .into()
        }
    }

    /// Register subscriptions for this application.
    ///
    /// Subscriptions are long-lived async tasks running in the background which
    /// emit messages to the application through a channel. They may be conditionally
    /// activated by selectively appending to the subscription batch, and will
    /// continue to execute for the duration that they remain in the batch.
    fn subscription(&self) -> Subscription<Self::Message> {
        Subscription::batch(vec![
            rectangle_tracker::subscription(0)
                .map(|(_sub_id, update)| Message::RectanglesUpdated(update)),
        ])
    }

    /// Handles messages emitted by the application and its widgets.
    ///
    /// Tasks may be returned for asynchronous execution of code in the background
    /// on the application's async runtime. The application will not exit until all
    /// tasks are finished.
    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::DiscoverBridge => {
                self.is_scanning = true;
                return Task::perform(
                    async {
                        let bridges =
                            huelib::bridge::discover_nupnp().map_err(|e| {
                                println!("Error discovering bridges: {}", e);
                                e.to_string()
                            })?;

                        println!("Bridges discovered: {:?}", bridges);

                        bridges
                            .first()
                            .map(|bridge| bridge.to_canonical())
                            .ok_or_else(|| fl!("no-bridge-found").to_string())
                    },
                    |result| Action::App(Message::BridgeDiscoveryFinished(result)),
                );
            }
            Message::BridgeDiscoveryFinished(Ok(bridge_ip)) => {
                self.is_scanning = false;

                self.last_discovery = Some(Ok(bridge_ip.clone()));
                if let Ok(ctx) = cosmic_config::Config::new(Self::APP_ID, Config::VERSION) {
                    let _ = self.config.set_bridge_ip(&ctx, Some(bridge_ip));
                }
            }
            Message::BridgeDiscoveryFinished(Err(error)) => {
                self.is_scanning = false;

                self.last_discovery = Some(Err(error));
            }
            Message::TogglePopup => {
                return if let Some(p) = self.popup.take() {
                    destroy_popup(p)
                } else {
                    let new_id = Id::unique();
                    self.popup.replace(new_id);
                    let mut popup_settings = self.core.applet.get_popup_settings(
                        self.core.main_window_id().unwrap(),
                        new_id,
                        None,
                        None,
                        None,
                    );
                    popup_settings.positioner.size_limits = Limits::NONE
                        .max_width(500.0)
                        .min_width(500.0)
                        .min_height(200.0)
                        .max_height(1080.0);

                    let open_popup = get_popup(popup_settings);

                    let maybe_load = if self.config.get_username().is_some() {
                        Task::batch([
                            Task::perform(async {}, |_| Action::App(Message::LoadLights)),
                            Task::perform(async {}, |_| Action::App(Message::LoadGroups)),
                            Task::perform(async {}, |_| Action::App(Message::LoadScenes)),
                        ])
                        // Task::perform(async {}, |_| Action::App(Message::LoadLights))
                    } else {
                        Task::none()
                    };

                    Task::batch([open_popup, maybe_load])
                };
            }
            Message::PopupClosed(id) => {
                if self.popup.as_ref() == Some(&id) {
                    self.popup = None;
                }
            }
            Message::PairBridge => {
                let bridge_ip = match self.config.get_bridge_ip() {
                    Some(bridge_ip) => bridge_ip.to_owned(),
                    None => return Task::none(),
                };
                return Task::perform(
                    async move {
                        huelib::bridge::register_user(bridge_ip, "cosmic-hue")
                            .map_err(|e| e.to_string())
                    },
                    |result| Action::App(Message::PairBridgeFinished(result)),
                );
            }
            Message::PairBridgeFinished(Ok(username)) => {
                if let Ok(ctx) = cosmic_config::Config::new(Self::APP_ID, Config::VERSION) {
                    let _ = self.config.set_username(&ctx, Some(username));
                };
                return Task::perform(async {}, |_| Action::App(Message::LoadLights));
            }
            Message::PairBridgeFinished(Err(error)) => {
                self.last_discovery = Some(Err(error));
            }
            Message::ToggleLightsMenu => {
                self.lights_menu_expanded = !self.lights_menu_expanded;
                if self.lights_menu_expanded {
                    self.groups_menu_expanded = false;
                    self.scenes_menu_expanded = false;
                }
            }
            Message::ToggleGroupsMenu => {
                self.groups_menu_expanded = !self.groups_menu_expanded;
                if self.groups_menu_expanded {
                    self.lights_menu_expanded = false;
                    self.scenes_menu_expanded = false;
                }
            }
            Message::ToggleScenesMenu => {
                self.scenes_menu_expanded = !self.scenes_menu_expanded;
                if self.scenes_menu_expanded {
                    self.lights_menu_expanded = false;
                    self.groups_menu_expanded = false;
                }
            }
            Message::LoadLights => {
                let bridge = match get_bridge(&self.config) {
                    Some(bridge) => bridge,
                    None => return Task::none(),
                };
                return Task::perform(
                    async move {
                        let lights = bridge.get_all_lights().map_err(|e| e.to_string())?;
                        Ok(lights)
                    },
                    |result| Action::App(Message::LightsLoaded(result)),
                );
            }
            Message::LightsLoaded(Ok(lights)) => {
                println!("Lights loaded: {}", lights.len());
                let mut lights_vm: Vec<LightVm> = lights
                    .into_iter()
                    .map(|light| LightVm {
                        id: light.id,
                        name: light.name,
                        on: light.state.on,
                        brightness: light.state.brightness,
                        color: Some(hsv_to_rgb(
                            light.state.hue,
                            light.state.saturation,
                            light.state.brightness,
                        )),
                    })
                    .collect();

                // Trier par ordre alphabétique
                lights_vm.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                self.lights = lights_vm;
            }
            Message::LightsLoaded(Err(error)) => {
                println!("Error loading lights: {:?}", error);
            }
            Message::LoadGroups => {
                let bridge = match get_bridge(&self.config) {
                    Some(bridge) => bridge,
                    None => return Task::none(),
                };
                return Task::perform(
                    async move {
                        let groups = bridge.get_all_groups().map_err(|e| e.to_string())?;
                        Ok(groups)
                    },
                    |result| Action::App(Message::GroupsLoaded(result)),
                );
            }
            Message::GroupsLoaded(Ok(groups)) => {
                println!("Groups loaded: {}", groups.len());
                let mut groups_vm: Vec<GroupVm> = groups
                    .into_iter()
                    .map(|group| {
                        let mut first_light = None;
                        if let Some(light_id) = group.lights.first() {
                            if let Some(light) =
                                self.lights.iter().find(|light| light.id == *light_id)
                            {
                                first_light = Some(light);
                            }
                        }
                        let color = if first_light.is_some() {
                            first_light.unwrap().color
                        } else {
                            Some((0.0, 0.0, 0.0))
                        };
                        let brightness = if first_light.is_some() {
                            first_light.unwrap().brightness
                        } else {
                            None
                        };

                        GroupVm {
                            id: group.id,
                            name: group.name,
                            on: group.state.map(|state| state.any_on),
                            brightness,
                            color,
                            lights: group.lights,
                        }
                    })
                    .collect();

                // Trier par ordre alphabétique
                groups_vm.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                self.groups = groups_vm;
            }
            Message::GroupsLoaded(Err(error)) => {
                println!("Error loading groups: {:?}", error);
            }
            Message::LoadScenes => {
                let bridge = match get_bridge(&self.config) {
                    Some(bridge) => bridge,
                    None => return Task::none(),
                };
                return Task::perform(
                    async move {
                        let scenes = bridge.get_all_scenes().map_err(|e| e.to_string())?;
                        Ok(scenes)
                    },
                    |result| Action::App(Message::ScenesLoaded(result)),
                );
            }
            Message::ScenesLoaded(Ok(scenes)) => {
                println!("Scenes loaded: {}", scenes.len());
                let mut scenes_vm: Vec<SceneVm> = scenes
                    .into_iter()
                    .map(|scene| SceneVm {
                        id: scene.id,
                        name: scene.name,
                        group: scene.group.unwrap_or_else(String::new),
                    })
                    .collect();

                // Trier par ordre alphabétique
                scenes_vm.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                self.scenes = scenes_vm;
            }
            Message::ScenesLoaded(Err(error)) => {
                println!("Error loading scenes: {:?}", error);
            }
            Message::ToggleLight(light_id, new_state) => {
                if let Some(light) = self.lights.iter_mut().find(|light| light.id == light_id) {
                    light.on = Some(new_state);
                }
                let bridge = match get_bridge(&self.config) {
                    Some(bridge) => bridge,
                    None => return Task::none(),
                };
                let modifier = huelib::resource::light::StateModifier::new().with_on(new_state);
                return Task::perform(
                    async move { bridge.set_light_state(&light_id, &modifier) },
                    |result| {
                        Action::App(Message::ResponsesModified(
                            result.map_err(|e| e.to_string()),
                        ))
                    },
                );
            }
            Message::ToggleGroup(group_id, new_state) => {
                if let Some(group) = self.groups.iter_mut().find(|group| group.id == group_id) {
                    println!("ToogleGroup: {}, new_state: {}", group_id, new_state);
                    group.on = Some(new_state);
                    group.lights.iter().for_each(|light_id| {
                        if let Some(light) =
                            self.lights.iter_mut().find(|light| light.id == *light_id)
                        {
                            light.on = Some(new_state);
                        }
                    });
                }
                let bridge = match get_bridge(&self.config) {
                    Some(bridge) => bridge,
                    None => return Task::none(),
                };
                let modifier = huelib::resource::group::StateModifier::new().with_on(new_state);
                return Task::perform(
                    async move { bridge.set_group_state(&group_id, &modifier) },
                    |result| {
                        Action::App(Message::ResponsesModified(
                            result.map_err(|e| e.to_string()),
                        ))
                    },
                );
            }
            Message::ActivateScene(scene_id) => {
                if let Some(scene) = self.scenes.iter().find(|scene| scene.id == scene_id) {
                    let modifier =
                        huelib::resource::group::StateModifier::new().with_scene(scene_id.clone());
                    let bridge = match get_bridge(&self.config) {
                        Some(bridge) => bridge,
                        None => return Task::none(),
                    };
                    let group_id = scene.group.clone();
                    return Task::perform(
                        async move { bridge.set_group_state(&group_id, &modifier) },
                        |result| {
                            Action::App(Message::SceneActivated(result.map_err(|e| e.to_string())))
                        },
                    );
                }
                return Task::none();
            }
            Message::SceneActivated(Ok(responses)) => {
                return Task::batch(vec![
                    Task::perform(async move { Ok(responses) }, |result| Action::App(Message::ResponsesModified(result))),
                    Task::perform(
                        async move {
                            // Wait for 10 seconds to reload light states to avoid intermediate values
                            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                        },
                        |_| Action::App(Message::LoadGroups),
                    ),
                    Task::perform(
                        async move {
                            // Wait for 10 seconds to reload light states to avoid intermediate values
                            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                        },
                        |_| Action::App(Message::LoadLights),
                    ),
                ]);
            }
            Message::SceneActivated(Err(error)) => {
                println!("Error activating scene: {:?}", error);
            }
            Message::ResponsesModified(Ok(responses)) => {
                println!("ResponsesModified: {:?}", responses);
            }
            Message::ResponsesModified(Err(error)) => {
                println!("Error: {:?}", error);
            }
            Message::SetLightBrightness(light_id, new_brightness) => {
                // Update the local state immediately
                if let Some(light) = self.lights.iter_mut().find(|light| light.id == light_id) {
                    light.brightness = Some(new_brightness as u8);
                }
                
                // Increment the counter and store the value in pending
                self.debounce_counter += 1;
                let counter = self.debounce_counter;
                self.pending_light_brightness.insert(light_id.clone(), (new_brightness, counter));
                
                // Create a task that will wait 300ms then apply the change
                return Task::perform(
                    async move {
                        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                        (light_id, counter)
                    },
                    |(id, cnt)| Action::App(Message::ApplyLightBrightness(id, cnt)),
                );
            }
            Message::ApplyLightBrightness(light_id, counter) => {
                // Check if it's still the last request
                if let Some((brightness, current_counter)) = self.pending_light_brightness.get(&light_id) {
                    if *current_counter == counter {
                        // It's the last request, apply it
                        let brightness = *brightness;
                        self.pending_light_brightness.remove(&light_id);
                        
                        let bridge = match get_bridge(&self.config) {
                            Some(bridge) => bridge,
                            None => return Task::none(),
                        };
                        let modifier = huelib::resource::light::StateModifier::new().with_brightness(
                            huelib::resource::Adjust::Override((brightness as u8).into()),
                        );
                        return Task::perform(
                            async move { bridge.set_light_state(&light_id, &modifier) },
                            |result| {
                                Action::App(Message::ResponsesModified(
                                    result.map_err(|e| e.to_string()),
                                ))
                            },
                        );
                    }
                }
            }
            Message::SetGroupBrightness(group_id, new_brightness) => {
                // Update the local state immediately
                if let Some(group) = self.groups.iter_mut().find(|group| group.id == group_id) {
                    if group.brightness.is_some() {
                        // Find all lights in the group and set their brightness to the new brightness
                        for light_id in &group.lights {
                            if let Some(light) =
                                self.lights.iter_mut().find(|light| light.id == *light_id)
                            {
                                light.brightness = Some(new_brightness as u8);
                            }
                        }

                        group.brightness = Some(new_brightness as u8);
                    }
                }
                
                // Increment the counter and store the value in pending
                self.debounce_counter += 1;
                let counter = self.debounce_counter;
                self.pending_group_brightness.insert(group_id.clone(), (new_brightness, counter));
                
                // Create a task that will wait 300ms then apply the change
                return Task::perform(
                    async move {
                        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                        (group_id, counter)
                    },
                    |(id, cnt)| Action::App(Message::ApplyGroupBrightness(id, cnt)),
                );
            }
            Message::ApplyGroupBrightness(group_id, counter) => {
                // Check if it's still the last request
                if let Some((brightness, current_counter)) = self.pending_group_brightness.get(&group_id) {
                    if *current_counter == counter {
                        // It's the last request, apply it
                        let brightness = *brightness;
                        self.pending_group_brightness.remove(&group_id);
                        
                        let bridge = match get_bridge(&self.config) {
                            Some(bridge) => bridge,
                            None => return Task::none(),
                        };
                        let modifier = huelib::resource::group::StateModifier::new().with_brightness(
                            huelib::resource::Adjust::Override((brightness as u8).into()),
                        );
                        return Task::perform(
                            async move { bridge.set_group_state(&group_id, &modifier) },
                            |result| {
                                Action::App(Message::ResponsesModified(
                                    result.map_err(|e| e.to_string()),
                                ))
                            },
                        );
                    }
                }
            }
            Message::SetLightColor(update) => {
                let _ = self.color_picker_model.update::<Message>(update.clone());
                if let Some(light) = self.lights.iter_mut().find(|light| {
                    Some(&light.id) == self.active_color_picker_item.as_ref().map(|(id, _)| id)
                }) {
                    match update {
                        ColorPickerUpdate::ActiveColor(color) => {
                            // Update the local state immediately for the UI
                            light.color = Some(hsv_palette_to_rgb(color));
                            
                            // Convert the HSV values
                            let (hue, saturation, brightness) = hsv_palette_to_hsv_lib(color);
                            
                            // Increment the counter and store the value in pending
                            let light_id = light.id.clone();
                            self.debounce_counter += 1;
                            let counter = self.debounce_counter;
                            self.pending_light_color.insert(light_id.clone(), ((hue, saturation, brightness), counter));
                            
                            // Create a task that will wait 300ms then apply the change
                            return Task::perform(
                                async move {
                                    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                                    (light_id, counter)
                                },
                                |(id, cnt)| Action::App(Message::ApplyLightColor(id, cnt)),
                            );
                        }
                        _ => {}
                    }
                }
            }
            Message::ApplyLightColor(light_id, counter) => {
                // Check if it's still the last request
                if let Some((color_values, current_counter)) = self.pending_light_color.get(&light_id) {
                    if *current_counter == counter {
                        // It's the last request, apply it
                        let (hue, saturation, brightness) = *color_values;
                        self.pending_light_color.remove(&light_id);
                        
                        let bridge = match get_bridge(&self.config) {
                            Some(bridge) => bridge,
                            None => return Task::none(),
                        };
                        
                        let modifier = huelib::resource::light::StateModifier::new()
                            .with_hue(huelib::resource::Adjust::Override(hue))
                            .with_saturation(huelib::resource::Adjust::Override(saturation))
                            .with_brightness(huelib::resource::Adjust::Override(brightness));
                        
                        return Task::perform(
                            async move { bridge.set_light_state(light_id, &modifier) },
                            |result| {
                                Action::App(Message::ResponsesModified(
                                    result.map_err(|e| e.to_string()),
                                ))
                            },
                        );
                    }
                }
            }
            Message::SetGroupColor(update) => {
                let _ = self.color_picker_model.update::<Message>(update.clone());
                if let Some(group) = self.groups.iter_mut().find(|group| {
                    Some(&group.id) == self.active_color_picker_item.as_ref().map(|(id, _)| id)
                }) {
                    match update {
                        ColorPickerUpdate::ActiveColor(color) => {
                            // Update the local state immediately for the UI
                            let new_color = hsv_palette_to_rgb(color);
                            for light_id in &group.lights {
                                if let Some(light) =
                                    self.lights.iter_mut().find(|light| light.id == *light_id)
                                {
                                    light.color = Some(new_color);
                                }
                            }
                            group.color = Some(new_color);

                            // Convert the HSV values
                            let (hue, saturation, brightness) = hsv_palette_to_hsv_lib(color);
                            
                            // Increment the counter and store the value in pending
                            let group_id = group.id.clone();
                            self.debounce_counter += 1;
                            let counter = self.debounce_counter;
                            self.pending_group_color.insert(group_id.clone(), ((hue, saturation, brightness), counter));
                            
                            // Create a task that will wait 300ms then apply the change
                            return Task::perform(
                                async move {
                                    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                                    (group_id, counter)
                                },
                                |(id, cnt)| Action::App(Message::ApplyGroupColor(id, cnt)),
                            );
                        }
                        _ => {}
                    }
                }
            }
            Message::ApplyGroupColor(group_id, counter) => {
                // Check if it's still the last request
                if let Some((color_values, current_counter)) = self.pending_group_color.get(&group_id) {
                    if *current_counter == counter {
                        // C'est la dernière demande, on l'applique
                        let (hue, saturation, brightness) = *color_values;
                        self.pending_group_color.remove(&group_id);
                        
                        let bridge = match get_bridge(&self.config) {
                            Some(bridge) => bridge,
                            None => return Task::none(),
                        };
                        
                        let modifier = huelib::resource::group::StateModifier::new()
                            .with_hue(huelib::resource::Adjust::Override(hue))
                            .with_saturation(huelib::resource::Adjust::Override(saturation))
                            .with_brightness(huelib::resource::Adjust::Override(brightness));
                        
                        return Task::perform(
                            async move { bridge.set_group_state(group_id, &modifier) },
                            |result| {
                                Action::App(Message::ResponsesModified(
                                    result.map_err(|e| e.to_string()),
                                ))
                            },
                        );
                    }
                }
            }
            Message::TryToggleColorPicker => {
                if self.color_button_rectangles.get(&0).is_none() {
                    return Task::perform(async {}, |_| Action::App(Message::TryToggleColorPicker));
                } else {
                    return if let Some(p) = self.color_picker_popup.take() {
                        self.color_button_rectangles.remove(&0);
                        destroy_popup(p)
                    } else {
                        self.open_color_picker_popup()
                    };
                }
            }
            Message::ToggleColorPicker((item_id, item_type)) => {
                let i_id = item_id.clone();
                let i_id_clone = i_id.clone();
                let i_type = item_type.clone();
                if self.active_color_picker_item == Some((i_id, i_type)) {
                    self.active_color_picker_item = None;
                } else {
                    self.active_color_picker_item = Some((item_id, item_type));
                }
                return if let Some(p) = self.color_picker_popup.take() {
                    destroy_popup(p)
                } else {
                    // The RectanglesUpdated will trigger the popup opening
                    if self.last_active_color_picker_item == Some(i_id_clone) {
                        self.open_color_picker_popup()
                    } else {
                        Task::none()
                    }
                };
            }
            Message::ToggleMoreMenu => {
                return if let Some(p) = self.more_menu_popup.take() {
                    destroy_popup(p)
                } else {
                    let new_id = Id::unique();
                    self.more_menu_popup.replace(new_id);
                    let mut more_menu_popup_settings = self.core.applet.get_popup_settings(
                        self.popup.unwrap(),
                        new_id,
                        None,
                        None,
                        None,
                    );
                    more_menu_popup_settings.positioner.size_limits = Limits::NONE
                        .max_width(200.0)
                        .min_width(120.0)
                        .min_height(120.0)
                        .max_height(200.0);

                    more_menu_popup_settings.positioner.anchor = Anchor::TopRight;
                    more_menu_popup_settings.positioner.gravity = Gravity::BottomRight;
                    more_menu_popup_settings.positioner.offset = (100, 50);

                    get_popup(more_menu_popup_settings)
                };
            }
            Message::RectanglesUpdated(update) => match update {
                RectangleUpdate::Init(tracker) => {
                    self.color_button_tracker = Some(tracker);
                }
                RectangleUpdate::Rectangle((id, rectangle)) => {
                    self.color_button_rectangles.insert(id, rectangle);
                    return Task::perform(async {}, |_| Action::App(Message::TryToggleColorPicker));
                }
            },
            Message::UnpairBridge => {
                if let Ok(ctx) = cosmic_config::Config::new(Self::APP_ID, Config::VERSION) {
                    let _ = self.config.set_bridge_ip(&ctx, None);
                    let _ = self.config.set_username(&ctx, None);
                    self.lights = Vec::new();
                    self.groups = Vec::new();
                    self.scenes = Vec::new();
                    if let Some(p) = self.more_menu_popup.take() {
                        return destroy_popup(p);
                    }
                };
            }
        }
        Task::none()
    }

    fn style(&self) -> Option<cosmic::iced_runtime::Appearance> {
        Some(cosmic::applet::style())
    }
}

impl AppModel {
    /// Build the lights section with header and light controls
    fn build_lights_section<'a>(&'a self) -> Element<'a, Message> {
        let lights_header = widget::flex_row(vec![
            widget::text::heading(fl!("lights"))
                .align_y(Alignment::Center)
                .height(30.0)
                .into(),
            widget::horizontal_space().into(),
            widget::button::icon(widget::icon::from_name(if self.lights_menu_expanded {
                "pan-up-symbolic"
            } else {
                "pan-down-symbolic"
            }))
            .on_press(Message::ToggleLightsMenu)
            .into(),
        ]);

        if self.lights_menu_expanded {
            if self.lights.is_empty() {
                return widget::flex_row(vec![lights_header.into(), widget::text(fl!("no-lights-found")).into()]).into();
            }

            let children: Vec<_> = self
                .lights
                .iter()
                .map(|light| self.build_light_item(light).padding(10).into())
                .collect();

            let content =
                widget::scrollable(widget::column::with_children(children).spacing(0)).spacing(10)
                    .height(Length::Fixed(600.0));

            widget::flex_row(vec![lights_header.into(), content.into()]).into()
        } else {
            widget::flex_row(vec![lights_header.into()]).into()
        }
    }

    /// Build a single light item with controls
    fn build_light_item<'a>(&'a self, light: &'a LightVm) -> widget::Column<'a, Message> {
        if let Some(on) = light.on {
            let name_toggle_row = widget::flex_row(vec![
                widget::text(&light.name).into(),
                widget::horizontal_space().into(),
                widget::toggler(on)
                    .on_toggle(|new_state| Message::ToggleLight(light.id.clone(), new_state))
                    .into(),
            ]);

            let (light_brightness, light_brightness_percent) = match light.brightness {
                Some(bri) => (
                    bri as f32,
                    ((bri.saturating_sub(1) as f32) / 253.0 * 100.0).round(),
                ),
                None => (0.0, 0.0),
            };

            // Get the current color as RGB (if available)
            let (r, g, b) = if let Some((r, g, b)) = light.color {
                (r, g, b)
            } else {
                (0.0, 0.0, 0.0)
            };

            let color_button = widget::color_picker::color_button(
                Some(Message::ToggleColorPicker((
                    light.id.clone(),
                    "light".to_string(),
                ))),
                Some(iced_core::Color::from_rgb(r, g, b)),
                Length::Fixed(32.0),
            );

            let color_button =
                if self.active_color_picker_item == Some((light.id.clone(), "light".to_string())) {
                    if let Some(tracker) = &self.color_button_tracker {
                        tracker.container(0u32, color_button).into()
                    } else {
                        color_button.into()
                    }
                } else {
                    color_button.into()
                };

            let slider_color_row = widget::flex_row(vec![
                widget::slider(1.0..=254.0, light_brightness, |new_brightness| {
                    Message::SetLightBrightness(light.id.clone(), new_brightness)
                })
                .into(),
                widget::text(format!("{}%", light_brightness_percent)).into(),
                widget::horizontal_space().into(),
                color_button,
            ]);

            widget::column::column()
                .width(Length::Fill)
                .spacing(10.0)
                .push(name_toggle_row)
                .push(slider_color_row)
        } else {
            widget::column::column().push(widget::settings::item(
                &light.name,
                widget::text(fl!("light-has-no-state", name = light.name.clone())),
            ))
        }
    }

    /// Build the groups section with header and group controls
    fn build_groups_section<'a>(&'a self) -> Element<'a, Message> {
        let groups_header = widget::flex_row(vec![
            widget::text::heading(fl!("groups"))
                .align_y(Alignment::Center)
                .height(30.0)
                .into(),
            widget::horizontal_space().into(),
            widget::button::icon(widget::icon::from_name(if self.groups_menu_expanded {
                "pan-up-symbolic"
            } else {
                "pan-down-symbolic"
            }))
            .on_press(Message::ToggleGroupsMenu)
            .into(),
        ]);

        if self.groups_menu_expanded {
            if self.groups.is_empty() {
                return widget::flex_row(vec![groups_header.into(), widget::text(fl!("no-groups-found")).into()]).into();
            }

            let children: Vec<_> = self
                .groups
                .iter()
                .map(|group| self.build_group_item(group).padding(10).into())
                .collect();

            let content =
                widget::scrollable(widget::column::with_children(children).spacing(0))
                    .height(Length::Fixed(600.0));

            widget::flex_row(vec![groups_header.into(), content.into()]).into()
        } else {
            widget::flex_row(vec![groups_header.into()]).into()
        }
    }

    /// Build a single group item with controls
    fn build_group_item<'a>(&'a self, group: &'a GroupVm) -> widget::Column<'a, Message> {
        if let Some(on) = group.on {
            let name_toggle_row = widget::flex_row(vec![
                widget::text(&group.name).into(),
                widget::horizontal_space().into(),
                widget::toggler(on)
                    .on_toggle(|new_state| Message::ToggleGroup(group.id.clone(), new_state))
                    .into(),
            ]);

            let (group_brightness, group_brightness_percent) = match group.brightness {
                Some(bri) => (
                    bri as f32,
                    ((bri.saturating_sub(1) as f32) / 253.0 * 100.0).round(),
                ),
                None => (0.0, 0.0),
            };

            // Get the current color as RGB (if available)
            let (r, g, b) = if let Some((r, g, b)) = group.color {
                (r, g, b)
            } else {
                (0.0, 0.0, 0.0)
            };

            let color_button = widget::color_picker::color_button(
                Some(Message::ToggleColorPicker((
                    group.id.clone(),
                    "group".to_string(),
                ))),
                Some(iced_core::Color::from_rgb(r, g, b)),
                Length::Fixed(32.0),
            );

            let color_button =
                if self.active_color_picker_item == Some((group.id.clone(), "group".to_string())) {
                    if let Some(tracker) = &self.color_button_tracker {
                        tracker.container(0u32, color_button).into()
                    } else {
                        color_button.into()
                    }
                } else {
                    color_button.into()
                };

            let slider_color_row = widget::flex_row(vec![
                widget::slider(1.0..=254.0, group_brightness, |new_brightness| {
                    Message::SetGroupBrightness(group.id.clone(), new_brightness)
                })
                .into(),
                widget::text(format!("{}%", group_brightness_percent)).into(),
                widget::horizontal_space().into(),
                color_button,
            ]);

            widget::column::column()
                .spacing(10.0)
                .push(name_toggle_row)
                .push(slider_color_row)
        } else {
            widget::column::column().push(widget::settings::item(
                &group.name,
                widget::text(format!("Group {} has no state", group.name)),
            ))
        }
    }

    /// Build the scenes section with header and scene controls
    fn build_scenes_section<'a>(&'a self) -> Element<'a, Message> {
        let scenes_header = widget::flex_row(vec![
            widget::text::heading(fl!("scenes"))
                .align_y(Alignment::Center)
                .height(30.0)
                .into(),
            widget::horizontal_space().into(),
            widget::button::icon(widget::icon::from_name(if self.scenes_menu_expanded {
                "pan-up-symbolic"
            } else {
                "pan-down-symbolic"
            }))
            .on_press(Message::ToggleScenesMenu)
            .into(),
        ]);

        if self.scenes_menu_expanded {
            if self.scenes.is_empty() {
                return widget::flex_row(vec![scenes_header.into(), widget::text(fl!("no-scenes-found")).into()]).into();
            }

            let children: Vec<_> = self
                .scenes
                .iter()
                .map(|scene| self.build_scene_item(scene).padding(10).into())
                .collect();

            let content =
                widget::scrollable(widget::column::with_children(children).spacing(0))
                    .height(Length::Fixed(600.0));

            widget::flex_row(vec![scenes_header.into(), content.into()]).into()
        } else {
            widget::flex_row(vec![scenes_header.into()]).into()
        }
    }

    /// Build a single scene item with controls
    fn build_scene_item<'a>(&'a self, scene: &'a SceneVm) -> widget::FlexRow<'a, Message> {
        let group_name =
            if let Some(group) = self.groups.iter().find(|group| group.id == scene.group) {
                group.name.clone()
            } else {
                fl!("global").to_string()
            };
        let display_text = fl!("scene-name-group-name", name = scene.name.clone(), group_name = group_name);
        widget::flex_row(vec![
            widget::text(display_text)
                .align_y(Alignment::Center)
                .height(30.0)
                .into(),
            widget::horizontal_space().into(),
            widget::button::icon(widget::icon::from_name("pan-end-symbolic"))
                .on_press(Message::ActivateScene(scene.id.clone()))
                .into(),
        ])
    }



    fn open_color_picker_popup(&mut self) -> Task<cosmic::Action<Message>> {
        let new_id = Id::unique();
        self.color_picker_popup.replace(new_id);
        self.last_active_color_picker_item = self.active_color_picker_item.as_ref().map(|(id, _)| id.clone());
        let mut color_picker_popup_settings = self.core.applet.get_popup_settings(
            self.popup.unwrap(),
            new_id,
            None,
            None,
            None,
        );
        color_picker_popup_settings.positioner.size_limits = Limits::NONE
            .max_width(420.0)
            .min_width(360.0)
            .min_height(200.0)
            .max_height(1080.0);

        if let Some(rect) = self.color_button_rectangles.get(&0).copied() {
            let rect_i32: Rectangle<i32> = Rectangle {
                x: rect.x.round() as i32,
                y: rect.y.round() as i32,
                width: rect.width.round() as i32,
                height: rect.height.round() as i32,
            };

            color_picker_popup_settings.positioner.anchor_rect = rect_i32;
        }
        color_picker_popup_settings.positioner.anchor = Anchor::Bottom;
        color_picker_popup_settings.positioner.gravity = Gravity::Bottom;
        color_picker_popup_settings.positioner.offset = (0, 6);

        get_popup(color_picker_popup_settings)
    }
}

fn get_bridge(config: &Config) -> Option<huelib::Bridge> {
    let bridge_ip: IpAddr = match config.get_bridge_ip() {
        Some(bridge_ip) => *bridge_ip,
        None => return None,
    };
    let username = match config.get_username() {
        Some(username) => username.to_owned(),
        None => return None,
    };
    Some(huelib::Bridge::new(bridge_ip, username))
}

fn hsv_palette_to_hsv_lib(color: palette::Hsv) -> (u16, u8, u8) {
    (
        (color.hue.into_positive_degrees() / 360.0 * 65535.0) as u16,
        (color.saturation * 254.0) as u8,
        (color.value * 254.0) as u8,
    )
}

fn hsv_palette_to_rgb(color: palette::Hsv) -> (f32, f32, f32) {
    hsv_to_rgb(
        Some((color.hue.into_positive_degrees() / 360.0 * 65535.0) as u16),
        Some((color.saturation * 254.0) as u8),
        Some((color.value * 254.0) as u8),
    )
}

fn hsv_to_rgb(hue: Option<u16>, saturation: Option<u8>, brightness: Option<u8>) -> (f32, f32, f32) {
    if let (Some(hue), Some(sat), Some(bri)) = (hue, saturation, brightness) {
        // Convert from HSV (Hue 0-65535, Sat/Bri 0-254) to RGB
        let h = (hue as f32) * 360.0 / 65535.0;
        let s = (sat as f32) / 254.0;
        let v = (bri as f32) / 254.0;

        // HSV to RGB conversion
        let c = v * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = v - c;

        let (r1, g1, b1) = if (0.0..60.0).contains(&h) {
            (c, x, 0.0)
        } else if (60.0..120.0).contains(&h) {
            (x, c, 0.0)
        } else if (120.0..180.0).contains(&h) {
            (0.0, c, x)
        } else if (180.0..240.0).contains(&h) {
            (0.0, x, c)
        } else if (240.0..300.0).contains(&h) {
            (x, 0.0, c)
        } else {
            (c, 0.0, x)
        };

        (r1 + m, g1 + m, b1 + m)
    } else {
        (0.0, 0.0, 0.0) // fallback color
    }
}
