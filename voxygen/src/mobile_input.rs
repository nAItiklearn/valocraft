//! Touch controls for the Android client.
//!
//! This is deliberately independent of the game's action mappings: it turns a
//! small set of on-screen regions into the same `Event` values that the desktop
//! mouse and keyboard path already produces.  It therefore works with the
//! existing singleplayer/session code without changing the server protocol.
//!
//! Layout (portrait or landscape, in physical screen coordinates):
//! * left 45% / lower 65%: movement joystick
//! * upper-right / right side: look drag
//! * short tap on the look area: primary action (attack/use)
//! * right-bottom primary region: primary action
//! * the regions above it: jump, interact, inventory, map, escape
//!
//! The buttons are intentionally simple for the first native build; a visual
//! HUD can be layered on top without changing this translation layer.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use conrod_core::{
    event::Input as ConrodInput,
    input::{Button as ConrodButton, Motion, MouseButton as ConrodMouseButton},
};
use vek::Vec2;
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{ElementState, Touch, TouchPhase},
};

use crate::{game_input::GameInput, ui, window::Event};

#[derive(Default)]
pub struct MobileTouch {
    look: Option<LookTouch>,
    joystick: Option<JoystickTouch>,
    menu_touch: Option<u64>,
    buttons: HashMap<u64, GameInput>,
    movement: MovementState,
}

struct LookTouch {
    id: u64,
    last: PhysicalPosition<f64>,
    started: Instant,
    distance: f32,
}

struct JoystickTouch {
    id: u64,
    origin: PhysicalPosition<f64>,
}

#[derive(Default)]
struct MovementState {
    forward: bool,
    back: bool,
    left: bool,
    right: bool,
}

impl MobileTouch {
    #[allow(clippy::too_many_arguments)]
    pub fn handle(
        &mut self,
        touch: Touch,
        screen: PhysicalSize<u32>,
        scale_factor: f64,
        cursor_grabbed: bool,
        menu_open: bool,
        pan_sensitivity: u32,
        mouse_y_inversion: bool,
        events: &mut Vec<Event>,
    ) {
        let position = touch.location;
        let width = screen.width as f64;
        let height = screen.height as f64;

        match touch.phase {
            TouchPhase::Started => {
                if cursor_grabbed {
                    if let Some(input) = button_at(position, width, height) {
                        self.buttons.insert(touch.id, input);
                        events.push(Event::InputUpdate(input, true));
                    } else if self.joystick.is_none() && is_joystick_region(position, width, height)
                    {
                        self.joystick = Some(JoystickTouch {
                            id: touch.id,
                            origin: position,
                        });
                    } else if self.look.is_none() {
                        self.look = Some(LookTouch {
                            id: touch.id,
                            last: position,
                            started: Instant::now(),
                            distance: 0.0,
                        });
                    }
                } else {
                    self.menu_touch = Some(touch.id);
                    self.send_menu_position(position, screen, scale_factor, events);
                    events.push(Event::MouseButton(
                        winit::event::MouseButton::Left,
                        ElementState::Pressed,
                    ));
                    events.push(Event::Ui(ui::Event(ConrodInput::Press(
                        ConrodButton::Mouse(ConrodMouseButton::Left),
                    ))));
                }
            },
            TouchPhase::Moved => {
                if let Some(joystick) = &self.joystick
                    && joystick.id == touch.id
                {
                    let delta = Vec2::new(
                        position.x as f32 - joystick.origin.x as f32,
                        position.y as f32 - joystick.origin.y as f32,
                    );
                    self.update_movement(delta, events);
                } else if let Some(look) = &mut self.look
                    && look.id == touch.id
                {
                    let delta = Vec2::new(
                        position.x as f32 - look.last.x as f32,
                        position.y as f32 - look.last.y as f32,
                    );
                    look.last = position;
                    look.distance += delta.magnitude();
                    let sensitivity = pan_sensitivity as f32 / 100.0;
                    let y_sign = if mouse_y_inversion { -1.0 } else { 1.0 };
                    events.push(Event::CursorPan(Vec2::new(
                        delta.x * sensitivity,
                        delta.y * sensitivity * y_sign,
                    )));
                } else if !cursor_grabbed && self.menu_touch == Some(touch.id) {
                    self.send_menu_position(position, screen, scale_factor, events);
                }
            },
            TouchPhase::Ended | TouchPhase::Cancelled => {
                // Match by touch id.  A second finger ending must not take the
                // look/joystick state belonging to the first finger.
                if self
                    .joystick
                    .as_ref()
                    .is_some_and(|joystick| joystick.id == touch.id)
                {
                    self.release_movement(events);
                    self.joystick = None;
                } else if self.look.as_ref().is_some_and(|look| look.id == touch.id) {
                    if let Some(look) = self.look.take() {
                        // A short, stationary touch on the world is a primary
                        // action (attack/use).  Dragging is look only.
                        if look.distance < 24.0
                            && look.started.elapsed() < Duration::from_millis(350)
                        {
                            events.push(Event::InputUpdate(GameInput::Primary, true));
                            events.push(Event::InputUpdate(GameInput::Primary, false));
                        }
                    }
                } else if let Some(input) = self.buttons.remove(&touch.id) {
                    events.push(Event::InputUpdate(input, false));
                } else if self.menu_touch == Some(touch.id) {
                    self.send_menu_position(position, screen, scale_factor, events);
                    events.push(Event::MouseButton(
                        winit::event::MouseButton::Left,
                        ElementState::Released,
                    ));
                    events.push(Event::Ui(ui::Event(ConrodInput::Release(
                        ConrodButton::Mouse(ConrodMouseButton::Left),
                    ))));
                    self.menu_touch = None;
                }

                // The menu_open field is part of the desktop UI state; keep it
                // referenced so this module stays compiled with the same
                // feature set on both platforms.
                let _ = menu_open;
            },
        }
    }

    fn update_movement(&mut self, delta: Vec2<f32>, events: &mut Vec<Event>) {
        const DEADZONE: f32 = 18.0;
        let magnitude = delta.magnitude();
        let (x, y) = if magnitude < DEADZONE {
            (0.0, 0.0)
        } else {
            (delta.x / magnitude, delta.y / magnitude)
        };

        // The joystick's origin is the touch-down point, so a small circle is
        // enough; holding near an edge gives diagonal movement.
        let threshold = 0.35;
        self.set_movement(
            MovementState {
                forward: -y > threshold,
                back: y > threshold,
                left: -x > threshold,
                right: x > threshold,
            },
            events,
        );
    }

    fn set_movement(&mut self, next: MovementState, events: &mut Vec<Event>) {
        if self.movement.forward != next.forward {
            events.push(Event::InputUpdate(GameInput::MoveForward, next.forward));
        }
        if self.movement.back != next.back {
            events.push(Event::InputUpdate(GameInput::MoveBack, next.back));
        }
        if self.movement.left != next.left {
            events.push(Event::InputUpdate(GameInput::MoveLeft, next.left));
        }
        if self.movement.right != next.right {
            events.push(Event::InputUpdate(GameInput::MoveRight, next.right));
        }
        self.movement = next;
    }

    fn release_movement(&mut self, events: &mut Vec<Event>) {
        self.set_movement(MovementState::default(), events);
    }

    fn send_menu_position(
        &self,
        position: PhysicalPosition<f64>,
        screen: PhysicalSize<u32>,
        scale_factor: f64,
        events: &mut Vec<Event>,
    ) {
        let width = screen.width as f64 / scale_factor;
        let height = screen.height as f64 / scale_factor;
        let x = position.x / scale_factor - width / 2.0;
        let y = -(position.y / scale_factor - height / 2.0);
        events.push(Event::Ui(ui::Event(ConrodInput::Motion(
            Motion::MouseCursor { x, y },
        ))));
    }
}

fn is_joystick_region(position: PhysicalPosition<f64>, width: f64, height: f64) -> bool {
    position.x < width * 0.45 && position.y > height * 0.35
}

fn button_at(position: PhysicalPosition<f64>, width: f64, height: f64) -> Option<GameInput> {
    let x = position.x / width;
    let y = position.y / height;

    // Keep these regions away from the look area and the joystick.  They are
    // intentionally generous so that a thumb can find them without a visual
    // HUD in the first build.
    if x > 0.72 && y > 0.55 {
        Some(GameInput::Primary)
    } else if x > 0.72 && y > 0.28 && y <= 0.55 {
        Some(GameInput::Jump)
    } else if x > 0.52 && x <= 0.72 && y > 0.70 {
        Some(GameInput::Interact)
    } else if x > 0.86 && y < 0.16 {
        Some(GameInput::Inventory)
    } else if x > 0.78 && x <= 0.86 && y < 0.16 {
        Some(GameInput::Map)
    } else if x < 0.14 && y < 0.16 {
        Some(GameInput::Escape)
    } else {
        None
    }
}
