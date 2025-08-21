use bevy::{app::*, prelude::*};
use bevy::render::extract_resource::*;
use bevy::prelude::KeyCode::{self, *};

pub trait AppExtensions {
    fn spawn_single<T: Component + Default>(&mut self) -> &mut Self;
    fn init_extract_resource<T: Resource + FromWorld + ExtractResource>(&mut self) -> &mut Self;
}

impl AppExtensions for App {

    fn spawn_single<T: Component + Default>(&mut self) -> &mut Self {
        self.add_systems(Startup, |mut c: Commands| { c.spawn(T::default()); })
    }

    fn init_extract_resource<T: Resource + FromWorld + ExtractResource>(&mut self) -> &mut Self {
        self.init_resource::<T>();
        self.add_plugins(ExtractResourcePlugin::<T>::default())
    }
}

pub trait InputExtensions {
    const DIGIT_KEYS: &[KeyCode] = &[Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9];
    const FUNCTION_KEYS: &[KeyCode] = &[F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12, F13, F14, F15, F16];
    const CTRL: [KeyCode; 2] = [ControlLeft, ControlRight];

    fn first_just_pressed(&self, keys: &[KeyCode]) -> Option<(usize, KeyCode)>;
    fn first_pressed(&self, keys: &[KeyCode]) -> Option<(usize, KeyCode)>;
    fn just_pressed_digit(&self) -> Option<usize>;
    fn just_pressed_function(&self) -> Option<usize>;
    fn just_control_pressed(&self, key: KeyCode) -> bool;
    fn is_control_pressing(&self, key: KeyCode) -> bool;
}

impl InputExtensions for ButtonInput<KeyCode> {

    fn first_just_pressed(&self, keys: &[KeyCode]) -> Option<(usize, KeyCode)> {
        for (i, key) in keys.iter().enumerate() {
            if self.just_pressed(*key) {
                return Some((i, *key));
            }
        }
        None
    }

    fn first_pressed(&self, keys: &[KeyCode]) -> Option<(usize, KeyCode)> {
        for (i, key) in keys.iter().enumerate() {
            if self.pressed(*key) {
                return Some((i, *key));
            }
        }
        None
    }
    
    fn just_pressed_digit(&self) -> Option<usize> {
        self.first_just_pressed(Self::DIGIT_KEYS)
            .map(|(i, _digit_key)| i)
    }
    
    fn just_pressed_function(&self) -> Option<usize> {
        self.first_just_pressed(Self::FUNCTION_KEYS)
            .map(|(i, _function_key)| 1 + i)
    }

    fn just_control_pressed(&self, key: KeyCode) -> bool {
        (self.any_just_pressed(Self::CTRL) && self.pressed(key)) || 
        (self.any_pressed(Self::CTRL) && self.just_pressed(key))
    }
    
    fn is_control_pressing(&self, key: KeyCode) -> bool {
        self.any_pressed(Self::CTRL) && self.pressed(key)
    }
}
