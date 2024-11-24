//
// rlr
//
// Copyright 2021 - Manos Pitsidianakis <manos@pitsidianak.is>
//
// This file is part of rlr.
//
// rlr is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// rlr is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with rlr. If not, see <http://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: GPL-3.0-or-later

#![deny(
    rustdoc::redundant_explicit_links,
    unsafe_op_in_unsafe_fn,
    /* groups */
    clippy::correctness,
    clippy::suspicious,
    clippy::complexity,
    clippy::perf,
    clippy::cargo,
    clippy::nursery,
    clippy::style,
    /* restriction */
    clippy::dbg_macro,
    clippy::rc_buffer,
    clippy::as_underscore,
    clippy::assertions_on_result_states,
    /* rustdoc */
    rustdoc::broken_intra_doc_links,
    /* pedantic */
    clippy::cast_lossless,
    clippy::cast_possible_wrap,
    clippy::ptr_as_ptr,
    clippy::doc_markdown,
    clippy::expect_fun_call,
    clippy::or_fun_call,
    clippy::bool_to_int_with_if,
    clippy::borrow_as_ptr,
    clippy::cast_ptr_alignment,
    clippy::large_futures,
    clippy::waker_clone_wake,
    clippy::unused_enumerate_index,
    clippy::unnecessary_fallible_conversions,
    clippy::struct_field_names,
    clippy::manual_hash_one,
    clippy::into_iter_without_iter,
)]
#![allow(
    // allow redundant_static_lifetimes to be able to compile from Rust version 1.70.0
    clippy::redundant_static_lifetimes,
    clippy::imprecise_flops,
    clippy::suboptimal_flops,
)]
use std::{
    f64::consts::{FRAC_PI_2, PI},
    io::Write,
    rc::Rc,
    sync::Mutex,
};

use glib::{g_print, g_printerr};
use gtk::{
    cairo::{Context, FontSlant, FontWeight},
    gdk, gio, glib,
    prelude::*,
    AboutDialog, DrawingArea,
};

const APP_ID: &'static str = "com.github.epilys.rlr";

trait CairoContextExt {
    fn set_primary_color(&self, settings: &Settings);
    fn set_secondary_color(&self, settings: &Settings);
}

impl CairoContextExt for Context {
    fn set_primary_color(&self, settings: &Settings) {
        self.set_source_rgba(
            settings.primary_color.red(),
            settings.primary_color.green(),
            settings.primary_color.blue(),
            settings.primary_color.alpha(),
        );
    }

    fn set_secondary_color(&self, settings: &Settings) {
        self.set_source_rgba(
            settings.secondary_color.red(),
            settings.secondary_color.green(),
            settings.secondary_color.blue(),
            settings.secondary_color.alpha(),
        );
    }
}

const GSCHEMA_XML: &'static str =
    include_str!("../data/com.github.epilys.rlr.Settings.gschema.xml");

include!("logo.xpm.rs");

/// Encode rotation state/angles around the starting left side as the origin
/// point.
///
/// ```text
///                   North
///                    ^^^
///                    |3|
///                    |2|
///               >    |1|
///              /     |0|      \
///            -/      +-+       \
///           /          .       v
///       <--------+.........+-------->
/// West  < 3 2 1 0|     .   |0 1 2 3 > East
///       <--------+     .   +-------->
///                      .        /
///             ^      +++      /-
///              \     |0|     <
///               \    |1|
///                    |2|
///                    |3|
///                    vvv
///
///                    South
/// ```
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
enum Rotation {
    E = 0,
    S = 1,
    W = 2,
    N = 3,
}

impl Rotation {
    #[inline(always)]
    const fn is_rotated(self) -> bool {
        !matches!(self as u8, 0 | 2)
    }

    #[inline(always)]
    const fn is_reversed(self) -> bool {
        matches!(self as u8, 2 | 3)
    }

    #[inline(always)]
    fn next(&mut self) -> Option<(Option<bool>, Option<bool>)> {
        use Rotation::*;
        let (new_val, ret) = match *self {
            E => (S, None),
            S => (W, Some((false.into(), None))),
            W => (N, Some((true.into(), false.into()))),
            N => (E, Some((None, true.into()))),
        };
        *self = new_val;
        ret
    }
}

#[derive(Clone, Copy, Debug)]
enum Interval {
    None,
    Start(f64),
    Full(f64, f64),
}

impl Interval {
    #[inline(always)]
    const fn is_start(&self) -> bool {
        matches!(self, Self::Start(_))
    }
}

#[derive(Debug)]
struct Settings {
    obj: Option<gio::Settings>,
    primary_color: gdk::RGBA,
    secondary_color: gdk::RGBA,
    window_opacity: f64,
    font_size_factor: f64,
    changed_signal_id: Option<glib::signal::SignalHandlerId>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            obj: None,
            primary_color: gdk::RGBA::parse("#453c0f").unwrap(),
            secondary_color: gdk::RGBA::parse("#f6d32d").unwrap(),
            window_opacity: 0.8,
            font_size_factor: 1.0,
            changed_signal_id: None,
        }
    }
}

impl Settings {
    const PRIMARY_COLOR: &'static str = "primary-color";
    const SECONDARY_COLOR: &'static str = "secondary-color";
    const WINDOW_OPACITY: &'static str = "window-opacity";
    const FONT_SIZE_FACTOR: &'static str = "font-size-factor";
    const ALL_KEYS: &'static [(&'static str, &'static glib::VariantTy)] = &[
        (Self::PRIMARY_COLOR, glib::VariantTy::STRING),
        (Self::SECONDARY_COLOR, glib::VariantTy::STRING),
        (Self::WINDOW_OPACITY, glib::VariantTy::DOUBLE),
        (Self::FONT_SIZE_FACTOR, glib::VariantTy::DOUBLE),
    ];

    fn new() -> Result<Self, std::borrow::Cow<'static, str>> {
        let Some(default_schemas) = gio::SettingsSchemaSource::default() else {
            return Err("Could not load default GSettings schemas.".into());
        };
        let Some(gsettings_schema) = default_schemas.lookup(APP_ID, true) else {
            return Err(format!("GSettings schema with id {APP_ID} was not found.").into());
        };
        let keys = gsettings_schema.list_keys();
        {
            let mut missing_keys = vec![];
            for (required_key, _) in Self::ALL_KEYS {
                if !keys.iter().any(|k| k == required_key) {
                    missing_keys.push(required_key);
                }
            }
            if !missing_keys.is_empty() {
                return Err(format!(
                    "GSettings schema does not contain valid keys; found keys {:?} but the \
                     following keys are missing: {:?}.",
                    keys, missing_keys
                )
                .into());
            }
        }
        // Now that we have ensured the keys exist, we can look them up safely and check
        // that they have the correct data types.
        {
            let mut invalid_key_types = vec![];
            for (required_key, data_type) in Self::ALL_KEYS {
                let value_type = gsettings_schema.key(required_key).value_type();
                if value_type.as_ref() != *data_type {
                    invalid_key_types.push(format!(
                        "Expected {} for key {} but found {} instead.",
                        data_type, required_key, value_type
                    ));
                }
                if !invalid_key_types.is_empty() {
                    return Err(format!(
                        "GSettings schema contains invalid property types; the following errors \
                         were encountered:\n{}.",
                        invalid_key_types.join("\n")
                    )
                    .into());
                }
            }
        }
        let mut retval = Self::default();
        let settings = gio::Settings::new(APP_ID);
        retval.obj = Some(settings);
        retval.sync_read();
        Ok(retval)
    }

    fn sync_read(&mut self) {
        let Self {
            obj: Some(ref obj),
            ref mut primary_color,
            ref mut secondary_color,
            ref mut window_opacity,
            ref mut font_size_factor,
            changed_signal_id: _,
        } = self
        else {
            return;
        };
        let primary_color_s: String = obj.get(Self::PRIMARY_COLOR);
        if let Ok(val) = gdk::RGBA::parse(&primary_color_s) {
            *primary_color = val;
        } else {
            g_printerr!(
                "Invalid {} value: {:?}\n",
                Self::PRIMARY_COLOR,
                primary_color_s
            );
        }
        let secondary_color_s: String = obj.get(Self::SECONDARY_COLOR);
        if let Ok(val) = gdk::RGBA::parse(&secondary_color_s) {
            *secondary_color = val;
        } else {
            g_printerr!(
                "Invalid {} value: {:?}\n",
                Self::SECONDARY_COLOR,
                secondary_color_s
            );
        }
        *window_opacity = obj.get::<f64>(Self::WINDOW_OPACITY).clamp(0.01, 1.0);
        *font_size_factor = obj.get::<f64>(Self::FONT_SIZE_FACTOR).clamp(0.1, 10.0);
    }

    fn sync_write(&self) {
        let Self {
            obj: Some(ref obj),
            ref primary_color,
            ref secondary_color,
            ref window_opacity,
            ref font_size_factor,
            ref changed_signal_id,
        } = self
        else {
            return;
        };
        if let Some(sid) = changed_signal_id.as_ref() {
            obj.block_signal(sid);
        }
        _ = obj.set(Self::PRIMARY_COLOR, primary_color.to_str().as_str());
        _ = obj.set(Self::SECONDARY_COLOR, secondary_color.to_str().as_str());
        _ = obj.set(Self::WINDOW_OPACITY, *window_opacity);
        _ = obj.set(Self::FONT_SIZE_FACTOR, *font_size_factor);
        gio::Settings::sync();
        if let Some(sid) = changed_signal_id.as_ref() {
            obj.unblock_signal(sid);
        }
    }
}

#[derive(Debug)]
struct Rlr {
    position: (f64, f64),
    root_position: (i32, i32),
    width: i32,
    height: i32,
    p_dimens: Option<(i32, i32)>,
    freeze: bool,
    rotate: Rotation,
    protractor: bool,
    precision: bool,
    edit_angle_offset: bool,
    angle_offset: f64,
    interval: Interval,
    ppi: f64,
    scale_factor: i32,
    settings: Settings,
}

impl Default for Rlr {
    fn default() -> Self {
        let settings = match Settings::new() {
            Ok(settings) => settings,
            Err(error) => {
                g_printerr!("Could not load application settings. {error}\n");
                Settings::default()
            }
        };
        Self {
            position: (0., 0.),
            root_position: (0, 0),
            width: 500,
            height: 35,
            p_dimens: None,
            freeze: false,
            rotate: Rotation::E,
            protractor: false,
            precision: true,
            edit_angle_offset: false,
            angle_offset: 0.,
            interval: Interval::None,
            ppi: 72.,
            scale_factor: 1,
            settings,
        }
    }
}

fn draw_rlr(rlr: Rc<Mutex<Rlr>>, drar: &DrawingArea, cr: &Context) -> glib::Propagation {
    let lck = rlr.lock().unwrap();
    cr.set_font_size(
        lck.settings.font_size_factor * (8.0 / f64::from(lck.scale_factor)) * lck.ppi / 72.,
    );
    if lck.protractor {
        return lck.draw_douglas(drar, cr);
    }
    lck.draw_rlr(drar, cr)
}

impl Rlr {
    fn set_size(&self, window: &gtk::ApplicationWindow) {
        if self.protractor {
            let max = std::cmp::max(self.width, self.height);
            window.resize(max, max);
        } else {
            window.resize(self.width, self.height);
        }
    }

    fn calc_angle_of_point(&self, (xr, yr): (f64, f64)) -> f64 {
        if yr.abs() == 0. {
            if xr >= 0. {
                0.
            } else {
                PI
            }
        } else {
            2. * f64::atan(yr / (xr + (xr * xr + yr * yr).sqrt()))
        }
    }

    fn draw_douglas(&self, _drar: &DrawingArea, cr: &Context) -> glib::Propagation {
        let length: f64 = f64::from(std::cmp::min(self.width, self.height));
        let root_position = self.root_position;
        let root_position = (
            f64::from(root_position.0) - length / 2.,
            -1. * (f64::from(root_position.1) - length / 2.),
        );
        let (xr, yr) = root_position;
        let angle = self.calc_angle_of_point((xr, yr));
        cr.arc(
            length / 2.,
            length / 2.,
            length / 2.,
            0.,
            2. * std::f64::consts::PI,
        );
        // Make entire canvas transparent, before starting to fill in the protractor
        // circular disk area which will be opaque.
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
        cr.fill().expect("Invalid cairo surface state");
        cr.set_secondary_color(&self.settings);
        cr.arc(
            length / 2.0,
            length / 2.0,
            length / 2.0,
            0.,
            2. * std::f64::consts::PI,
        );
        cr.fill().expect("Invalid cairo surface state");

        let _pixels_per_tick = 10;
        let tick_size = 5.;
        cr.set_primary_color(&self.settings);
        cr.set_line_width(1.);

        cr.save().unwrap();
        cr.set_source_rgba(0.1, 0.1, 0.1, 0.1);

        // Make concentric circles at distance `tick_size`
        for i in 1..(length / 2.).floor() as i64 {
            let r = (i as f64) * tick_size * 10.;
            if 2. * r >= length {
                break;
            }
            cr.arc(length / 2., length / 2., r, 0., 2. * std::f64::consts::PI);
            cr.stroke().expect("Invalid cairo surface state");
        }
        cr.restore().unwrap();

        // Make circular angle ticks at the outmost circle
        for quadrant in 0..4 {
            let mut a: u64 = 0;
            // π * 0.5 == 1.57079...
            while a <= 157 {
                let tick_size = if ((a as f64) * (1.8 / PI)) % 30. <= 0.55 {
                    5.0 * tick_size
                } else if ((a as f64) * (1.8 / PI)) % 5. <= 0.5 {
                    1.5 * tick_size
                } else {
                    tick_size
                };
                cr.save().unwrap();
                cr.move_to(length / 2. - 0.5, length / 2. - 0.5);
                // cr.rotate(1.5 * PI + (quadrant as f64) * FRAC_PI_2);
                cr.rotate(f64::from(quadrant) * FRAC_PI_2);
                cr.rotate(-(a as f64 / 100.0));
                let cur = cr.current_point().unwrap();
                cr.move_to(cur.0 + length / 2. - 0.5 - tick_size, cur.1 - 0.5);
                cr.line_to(cur.0 + length / 2. - 0.5, cur.1 - 0.5);
                cr.stroke().expect("Invalid cairo surface state");
                cr.restore().unwrap();
                a += 1;
            }
        }

        // Make 0 radian radius (offsetted by `self.angle_offset`)
        cr.save().unwrap();
        cr.set_line_width(2.);
        cr.move_to(length / 2. - 0.5, length / 2. - 0.5);
        cr.rotate(2. * PI - FRAC_PI_2 - self.angle_offset);
        let cur = cr.current_point().unwrap();
        cr.line_to(cur.0, cur.1 + length / 2. - 0.5);
        cr.stroke().expect("Invalid cairo surface state");
        cr.restore().unwrap();

        // Draw radius tracking mouse position
        cr.save().unwrap();
        let _angle = if self.precision {
            angle + FRAC_PI_2
        } else {
            angle.round() + FRAC_PI_2
        };
        cr.move_to(length / 2. - 0.5, length / 2. - 0.5);
        cr.rotate(2. * PI - _angle);
        let cur = cr.current_point().unwrap();

        // Draw center point as a small circle
        cr.arc(cur.0, cur.1, 2., 0., 2. * std::f64::consts::PI);
        cr.stroke().expect("Invalid cairo surface state");
        cr.move_to(cur.0, cur.1);
        cr.line_to(cur.0, cur.1 + length / 2. - 0.5);
        cr.stroke().expect("Invalid cairo surface state");
        cr.restore().unwrap();
        cr.select_font_face("Sans", FontSlant::Normal, FontWeight::Normal);

        // Draw arc signifying which angle is being measured
        cr.move_to(length / 2. - 0.5, length / 2. - 0.5);
        let angle = if root_position.1 < 0. {
            (PI - angle.abs()) + PI - self.angle_offset
        } else {
            angle - self.angle_offset
        };
        cr.arc(
            length / 2.,
            length / 2.,
            17.,
            2. * PI - _angle + FRAC_PI_2,
            2. * PI - self.angle_offset,
        );
        cr.stroke().expect("Invalid cairo surface state");

        // Show angle measurement as text
        cr.move_to(length / 2. - 5.5, length / 2. - 15.5);
        cr.show_text(&format!(
            " {:.2}rad {:.2}°",
            if self.precision { angle } else { angle.round() },
            if self.precision { angle } else { angle.round() } * (180. / PI)
        ))
        .expect("Invalid cairo surface state");

        glib::Propagation::Proceed
    }

    fn draw_rlr(&self, _drar: &DrawingArea, cr: &Context) -> glib::Propagation {
        let position = self.position;
        let length: f64 = f64::from(self.width);
        let height: f64 = f64::from(self.height);
        let breadth = if self.rotate.is_rotated() {
            f64::from(self.width)
        } else {
            f64::from(self.height)
        };

        cr.set_secondary_color(&self.settings);
        cr.paint().expect("Invalid cairo surface state");

        let _pixels_per_tick = 10;
        let tick_size = 5.;
        let mut i = 0;
        let mut x: f64;
        cr.set_line_width(0.5);
        cr.set_primary_color(&self.settings);
        cr.save().unwrap();
        match self.interval {
            Interval::Start(start_pos) => {
                cr.set_source_rgb(0.9, 0.9, 0.9);
                cr.rectangle(
                    start_pos - 0.5,
                    0.5,
                    position.0 - start_pos - 0.5,
                    breadth - 0.5,
                );
                cr.fill().expect("Invalid cairo surface state");
                cr.set_source_rgb(0.1, 0.1, 0.1);
                cr.rectangle(
                    start_pos - 0.5,
                    0.5,
                    position.0 - start_pos - 0.5,
                    breadth - 0.5,
                );
                cr.stroke().expect("Invalid cairo surface state");
            }
            Interval::Full(start_pos, end_pos) => {
                cr.set_source_rgb(0.8, 0.8, 0.8);
                cr.rectangle(
                    start_pos - 0.5,
                    0.5,
                    end_pos - 0.5 - start_pos,
                    breadth - 0.5,
                );
                cr.fill().expect("Invalid cairo surface state");
                cr.set_source_rgb(0.1, 0.1, 0.1);
                cr.rectangle(
                    start_pos - 0.5,
                    0.5,
                    end_pos - 0.5 - start_pos,
                    breadth - 0.5,
                );
                cr.stroke().expect("Invalid cairo surface state");
            }
            _ => {}
        }
        cr.restore().unwrap();
        cr.set_line_width(1.);
        let is_reversed = self.rotate.is_reversed();
        if self.rotate.is_rotated() {
            while i < self.height {
                x = f64::from(i).floor() + 0.5;
                if is_reversed {
                    x = height - x;
                }
                cr.move_to(1.0, x);
                let tick_size = if i % 50 == 0 {
                    tick_size * 1.5
                } else if i % 10 == 0 {
                    tick_size
                } else {
                    tick_size * 0.5
                };
                cr.line_to(tick_size, x);
                cr.stroke().expect("Invalid cairo surface state");
                cr.move_to(breadth - tick_size, x);
                cr.line_to(breadth - 1.0, x);
                cr.stroke().expect("Invalid cairo surface state");
                if i % 50 == 0 {
                    cr.select_font_face("Monospace", FontSlant::Normal, FontWeight::Normal);
                    let label = format!("{}", i * self.scale_factor);
                    let extents = cr
                        .text_extents(&label)
                        .expect("Invalid cairo surface state");
                    cr.move_to(breadth / 2. - 2.5 - extents.width() as f64 / 2., x);
                    cr.show_text(&label).expect("Invalid cairo surface state");
                }
                i += 2;
            }
            let pos = if self.precision {
                position.1.floor()
            } else {
                (position.1 / 10.).floor() * 10.
            };
            let x = pos + 0.5;
            cr.move_to(1.0, x);
            cr.line_to(breadth, x);
            cr.stroke().expect("Invalid cairo surface state");
            let pos_label = format!("{}px", pos * f64::from(self.scale_factor));
            let extents = cr
                .text_extents(&pos_label)
                .expect("Invalid cairo surface state");
            cr.rectangle(
                breadth / 2. - extents.width() as f64 / 2. - 2.,
                x - extents.height() as f64 - 2.,
                extents.width() as f64 + 6.5,
                extents.height() as f64 + 6.5,
            );
            cr.stroke().expect("Invalid cairo surface state");
            cr.rectangle(
                breadth / 2. - extents.width() as f64 / 2.,
                x - extents.height() as f64,
                extents.width() as f64 + 4.5,
                extents.height() as f64 + 4.5,
            );
            cr.set_secondary_color(&self.settings);
            cr.fill().expect("Invalid cairo surface state");
            cr.set_primary_color(&self.settings);

            cr.select_font_face("Sans", FontSlant::Normal, FontWeight::Normal);

            cr.move_to(breadth / 2. - extents.width() as f64 / 2., x);
            cr.show_text(&pos_label)
                .expect("Invalid cairo surface state");

            cr.rectangle(0.5, 0.5, length - 1.0, height - 1.0);
        } else {
            while i < self.width {
                x = f64::from(i).floor() + 0.5;
                if is_reversed {
                    x = length - x;
                }
                cr.move_to(x, 1.0);
                let tick_size = if i % 50 == 0 {
                    tick_size * 1.5
                } else if i % 10 == 0 {
                    tick_size
                } else {
                    tick_size * 0.5
                };
                cr.line_to(x, tick_size);
                cr.stroke().expect("Invalid cairo surface state");
                cr.move_to(x, breadth - tick_size);
                cr.line_to(x, breadth - 1.0);
                cr.stroke().expect("Invalid cairo surface state");
                if i % 50 == 0 {
                    cr.select_font_face("Monospace", FontSlant::Normal, FontWeight::Normal);
                    let label = format!("{}", i * self.scale_factor);
                    let extents = cr
                        .text_extents(&label)
                        .expect("Invalid cairo surface state");
                    cr.move_to(x - extents.width() as f64 / 2., breadth / 2. + 2.5);
                    cr.show_text(&label).expect("Invalid cairo surface state");
                }
                i += 2;
            }
            let pos = if self.precision {
                position.0.floor()
            } else {
                (position.0 / 10.).floor() * 10.
            };
            let x = pos + 0.5 + 2.0;
            cr.move_to(x - 2., 1.0);
            cr.line_to(x - 2., breadth);
            cr.stroke().expect("Invalid cairo surface state");

            let pos_label = format!("{}px", pos * f64::from(self.scale_factor));
            let extents = cr
                .text_extents(&pos_label)
                .expect("Invalid cairo surface state");
            cr.rectangle(
                x - 2.,
                breadth / 2. - extents.height() as f64 - 2.,
                extents.width() as f64 + 6.5,
                extents.height() as f64 + 10.5,
            );
            cr.stroke().expect("Invalid cairo surface state");
            cr.rectangle(
                x,
                breadth / 2. - extents.height() as f64,
                extents.width() as f64 + 4.5,
                extents.height() as f64 + 8.5,
            );
            cr.set_secondary_color(&self.settings);
            cr.fill().expect("Invalid cairo surface state");
            cr.set_primary_color(&self.settings);

            cr.select_font_face("Sans", FontSlant::Normal, FontWeight::Normal);

            cr.move_to(x, breadth / 2. + 2.5);
            cr.show_text(&pos_label)
                .expect("Invalid cairo surface state");

            cr.rectangle(0.5, 0.5, length - 1.0, breadth - 1.0);
        }
        cr.stroke().expect("Invalid cairo surface state");

        glib::Propagation::Proceed
    }
}

fn run_app() -> Option<i32> {
    let application = gtk::Application::new(Some(APP_ID), gio::ApplicationFlags::default());

    let rlr = Rc::new(Mutex::new(Rlr::default()));

    application.add_main_option(
        "install-gsettings-schema",
        b'\0'.into(),
        glib::OptionFlags::NONE,
        glib::OptionArg::String,
        "Install the application's setting schema to the given directory. The directory will not \
         be created if it doesn't exist. As a special case, if the directory value is \"-\" the \
         schema will be printed at standard output. In most systems the value given should be one \
         of [\"$HOME/.local/share/glib-2.0/schemas/\", \"/usr/share/glib-2.0/schemas/\"]. As a \
         reminder, the command `glib-compile-schemas /path/to/glib-2.0/schemas/` must be executed \
         for changes to take effect.",
        Some("GLIB_2_0_SCHEMAS_DIR"),
    );
    application.connect_handle_local_options(
        |_: &gtk::Application, options_dict: &glib::VariantDict| -> i32 {
            if let Some(dir) = options_dict
                .lookup_value("install-gsettings-schema", Some(glib::VariantTy::STRING))
                .and_then(|variant| Some(variant.str()?.to_string()))
            {
                match dir.as_str() {
                    "-" => {
                        g_print!("{}", GSCHEMA_XML);
                        return 0;
                    }
                    actual_path => {
                        let path = std::path::Path::new(actual_path);
                        let Ok(metadata) = std::fs::metadata(path) else {
                            g_printerr!(
                                "Directory {} either does not exist or you do not have \
                                 permissions to access it.\n",
                                actual_path
                            );
                            return 1;
                        };
                        if !metadata.is_dir() {
                            g_printerr!(
                                "Argument value {} is not actually a directory.\n",
                                actual_path
                            );
                            return 1;
                        }
                        let gschema_path = path.join(format!("{APP_ID}.Settings.gschema.xml"));
                        match std::fs::OpenOptions::new()
                            .write(true)
                            .create(true)
                            .truncate(true)
                            .open(&gschema_path)
                            .and_then(|mut file| file.write_all(GSCHEMA_XML.as_bytes()))
                        {
                            Err(err) => {
                                g_printerr!("Could not open {} for writing: {err}\n", actual_path);
                                return 1;
                            }
                            Ok(_) => {
                                g_print!(
                                    "Wrote schema to {}. You should run the following command to \
                                     compile the schema:\nglib-compile-schemas {actual_path}\n",
                                    gschema_path.display()
                                );
                            }
                        }
                        return 0;
                    }
                }
            }

            // Pretty print:
            //
            // g_printerr!("{:?}", options_dict.end().print(true));
            -1
        },
    );

    application.connect_startup(|application: &gtk::Application| {
        application.set_accels_for_action("app.quit", &["<Primary>Q", "Q"]);
        application.set_accels_for_action("app.rotate", &["R"]);
        application.set_accels_for_action("app.flip", &["<Shift>R"]);
        application.set_accels_for_action("app.protractor", &["P"]);
        application.set_accels_for_action("app.freeze", &["F", "space"]);
        application.set_accels_for_action("app.increase", &["plus"]);
        application.set_accels_for_action("app.decrease", &["minus"]);
        application.set_accels_for_action("app.increase_font_size", &["<Primary>plus"]);
        application.set_accels_for_action("app.decrease_font_size", &["<Primary>minus"]);
        application.set_accels_for_action("app.about", &["question", "F1"]);
        application.set_accels_for_action("app.settings", &["s", "F2"]);
        application
            .set_accels_for_action("app.move_right", &["Right", "<Primary>Right", "rightarrow"]);
        application.set_accels_for_action("app.move_left", &["Left", "<Primary>Left", "leftarrow"]);
        application.set_accels_for_action("app.move_up", &["Up", "<Primary>Up", "uparrow"]);
        application.set_accels_for_action("app.move_down", &["Down", "<Primary>Down", "downarrow"]);
        application.set_accels_for_action("app.move_to_center", &["Home", "h"]);
    });
    application.connect_activate(move |application: &gtk::Application| {
        let _rlr = rlr.clone();
        let _rlr2 = rlr.clone();
        drawable(
            application,
            _rlr,
            move |drar: &DrawingArea, cr: &Context| -> glib::Propagation {
                let _rlr = _rlr2.clone();
                draw_rlr(_rlr, drar, cr)
            },
        );
    });

    let retval = application.run();
    if retval != glib::ExitCode::SUCCESS {
        Some(retval.value())
    } else {
        None
    }
}

fn main() {
    if let Some(exit_code) = run_app() {
        std::process::exit(exit_code);
    }
}

fn drawable<F>(application: &gtk::Application, rlr: Rc<Mutex<Rlr>>, draw_fn: F)
where
    F: Fn(&DrawingArea, &Context) -> glib::Propagation + 'static,
{
    let window = gtk::ApplicationWindow::builder()
        .application(application)
        .events(gdk::EventMask::POINTER_MOTION_MASK)
        .build();
    window.set_icon(Some(&gtk::gdk_pixbuf::Pixbuf::from_xpm_data(ICON).unwrap()));

    set_visual(&window, None);

    {
        let rlr2 = rlr.clone();
        let mut lck = rlr.lock().unwrap();
        let window = window.clone();
        lck.settings.changed_signal_id = lck.settings.obj.as_ref().map(|obj| {
            obj.connect_changed(None, move |_self: &gio::Settings, key: &str| {
                let rlr = rlr2.clone();
                let mut lck = rlr.lock().unwrap();
                lck.settings.sync_read();
                if key == Settings::WINDOW_OPACITY {
                    window.set_opacity(lck.settings.window_opacity);
                }
                drop(lck);
                window.queue_draw();
            })
        });
    }
    window.connect_screen_changed(set_visual);
    {
        let rlr = rlr.clone();
        let window = window.clone();
        let tick = move || {
            let mut lck = rlr.lock().unwrap();
            if lck.edit_angle_offset || lck.freeze {
                return glib::ControlFlow::Continue;
            }
            if let Some(screen) = window.window() {
                let root_origin = screen.root_origin();
                let Some(device) = screen
                    .display()
                    .default_seat()
                    .and_then(|seat| seat.pointer())
                else {
                    return glib::ControlFlow::Continue;
                };
                let (_, x, y) = device.position();
                let root_position = (x - root_origin.0, y - root_origin.1);

                if root_position != lck.root_position {
                    if lck.protractor {
                        lck.root_position = root_position;
                        lck.position.0 = f64::from(root_position.0);
                        lck.position.1 = f64::from(root_position.1);
                        drop(lck);
                        window.queue_draw();
                    } else if lck.rotate.is_rotated()
                        && root_position.1 < lck.height
                        && root_position.1 > 0
                    {
                        lck.root_position = root_position;
                        lck.position.1 = f64::from(root_position.1);
                        drop(lck);
                        window.queue_draw();
                    } else if !lck.rotate.is_rotated()
                        && root_position.0 < lck.width
                        && root_position.0 > 0
                    {
                        lck.root_position = root_position;
                        lck.position.0 = f64::from(root_position.0);
                        drop(lck);
                        window.queue_draw();
                    }
                }
            }
            glib::ControlFlow::Continue
        };

        // executes the closure once every second
        glib::timeout_add_local(std::time::Duration::from_millis(10), tick);
    }

    window.connect_enter_notify_event(enter_notify);
    window.connect_leave_notify_event(leave_notify);

    let _rlr = rlr.clone();
    window.connect_button_press_event(
        move |window: &gtk::ApplicationWindow, ev: &gtk::gdk::EventButton| -> glib::Propagation {
            let rlr = _rlr.clone();
            let mut lck = rlr.lock().unwrap();

            if ev.event_type() == gtk::gdk::EventType::ButtonPress && lck.interval.is_start() {
                if let Interval::Start(start_pos) = lck.interval {
                    lck.interval = Interval::Full(
                        start_pos,
                        if lck.rotate.is_rotated() {
                            ev.position().1
                        } else {
                            ev.position().0
                        },
                    );
                }
            } else if ev.event_type() == gtk::gdk::EventType::DoubleButtonPress {
                lck.interval = if lck.rotate.is_rotated() {
                    Interval::Start(ev.position().1)
                } else {
                    Interval::Start(ev.position().0)
                };
            } else if ev.button() == 1 && !lck.precision {
                lck.edit_angle_offset = true;
                drop(lck);
            } else if ev.button() == 1 {
                #[allow(clippy::cast_possible_wrap)]
                window.begin_move_drag(
                    ev.button() as i32,
                    ev.root().0 as i32,
                    ev.root().1 as i32,
                    ev.time(),
                );
            }
            glib::Propagation::Proceed
        },
    );
    let _rlr = rlr.clone();
    window.connect_button_release_event(
        move |_application: &gtk::ApplicationWindow,
              ev: &gtk::gdk::EventButton|
              -> glib::Propagation {
            let rlr = _rlr.clone();
            // g_printerr!("drag end\n");
            if ev.button() == 1 {
                rlr.lock().unwrap().edit_angle_offset = false;
            }
            glib::Propagation::Proceed
        },
    );
    let _rlr = rlr.clone();
    window.connect_key_press_event(
        move |window: &gtk::ApplicationWindow, ev: &gtk::gdk::EventKey| -> glib::Propagation {
            // g_printerr!("press {}\n", ev.keyval().name().unwrap().as_str());
            if ev
                .keyval()
                .name()
                .map(|n| n.as_str() == "Control_L")
                .unwrap_or(false)
            {
                let rlr = _rlr.clone();
                rlr.lock().unwrap().precision = false;
                window.queue_draw();
            }
            glib::Propagation::Proceed
        },
    );
    let _rlr = rlr.clone();
    window.connect_key_release_event(
        move |window: &gtk::ApplicationWindow, ev: &gtk::gdk::EventKey| -> glib::Propagation {
            // g_printerr!("release {}\n", ev.keyval().name().unwrap().as_str());
            if ev
                .keyval()
                .name()
                .map(|n| n.as_str() == "Control_L")
                .unwrap_or(false)
            {
                let rlr = _rlr.clone();
                rlr.lock().unwrap().precision = true;
                window.queue_draw();
            }
            glib::Propagation::Proceed
        },
    );
    let _rlr = rlr.clone();
    window.connect_motion_notify_event(
        move |window: &gtk::ApplicationWindow, motion: &gdk::EventMotion| -> glib::Propagation {
            let rlr = _rlr.clone();
            {
                let mut lck = rlr.lock().unwrap();
                lck.position = motion.position();
                if lck.edit_angle_offset {
                    let (xr, yr) = lck.position;
                    let translated_position = (
                        xr - f64::from(lck.width) / 2.,
                        f64::from(lck.width) / 2. - yr,
                    );
                    let angle = lck.calc_angle_of_point(translated_position);
                    lck.angle_offset = angle;
                }
            }
            window.queue_draw();
            glib::Propagation::Proceed
        },
    );
    let _rlr = rlr.clone();
    window.connect_configure_event(
        move |window: &gtk::ApplicationWindow, event: &gdk::EventConfigure| -> bool {
            let rlr = _rlr.clone();
            {
                let mut lck = rlr.lock().unwrap();
                lck.width = event.size().0.try_into().unwrap_or(i32::MAX);
                lck.height = event.size().1.try_into().unwrap_or(i32::MAX);
            }
            window.queue_draw();

            false
        },
    );
    window.set_app_paintable(true); // crucial for transparency
    window.set_resizable(true);
    window.set_decorated(false);
    // #[cfg(debug_assertions)]
    // gtk::Window::set_interactive_debugging(true);

    let drawing_area = DrawingArea::new();

    drawing_area.connect_draw(draw_fn);

    if let Ok(lck) = rlr.lock() {
        window.set_default_size(lck.width, lck.height);
    }

    window.add(&drawing_area);
    window.set_opacity(rlr.lock().unwrap().settings.window_opacity);

    build_system_menu(application);

    add_actions(application, &window, rlr.clone());

    window.show_all();
    let (ppi, scale_factor) = get_ppi_and_scale_factor(&window);
    if let Ok(mut lck) = rlr.lock() {
        if ppi > 72. {
            lck.ppi = ppi;
            lck.scale_factor = scale_factor;
            lck.width += (scale_factor * lck.width) / 2;
            lck.height += (scale_factor * lck.height) / 2;
            window.set_default_size(lck.width, lck.height);
            window.resize(lck.width, lck.height);
            window.queue_draw();
            // g_printerr!("resized to {}x{}\n", lck.width, lck.height);
        } else {
            lck.scale_factor = scale_factor;
        }
    }
}

fn get_ppi_and_scale_factor(window: &gtk::ApplicationWindow) -> (f64, i32) {
    const INCH: f64 = 0.0393701;

    let display = window.display();
    let monitor = display
        .monitor_at_window(&window.window().unwrap())
        .unwrap();
    let scale_factor = monitor.scale_factor();
    let width_mm = f64::from(monitor.width_mm());
    let height_mm = f64::from(monitor.height_mm());

    let rectangle = monitor.geometry();
    let width = f64::from(scale_factor) * f64::from(rectangle.width());
    let height = f64::from(scale_factor) * f64::from(rectangle.height());
    let diag = (width_mm * width_mm + height_mm * height_mm).sqrt() * INCH;

    (
        (width * width + height * height).sqrt() / diag,
        scale_factor,
    )
}

fn enter_notify(
    window: &gtk::ApplicationWindow,
    _crossing: &gtk::gdk::EventCrossing,
) -> glib::Propagation {
    // g_printerr!("enter\n");
    if let Some(screen) = window.window() {
        let display = screen.display();
        if let Some(gdk_window) = window.window() {
            gdk_window.set_cursor(Some(
                &gtk::gdk::Cursor::from_name(&display, "move").unwrap(),
            ));
        }
    }
    glib::Propagation::Proceed
}

const fn leave_notify(
    _application: &gtk::ApplicationWindow,
    _crossing: &gtk::gdk::EventCrossing,
) -> glib::Propagation {
    // g_printerr!("leave\n");
    glib::Propagation::Proceed
}

fn set_visual(window: &gtk::ApplicationWindow, _screen: Option<&gtk::gdk::Screen>) {
    if let Some(screen) = gtk::prelude::GtkWindowExt::screen(window) {
        if let Some(ref visual) = screen.rgba_visual() {
            window.set_visual(Some(visual)); // crucial for transparency
        }
    }
}

const fn build_system_menu(_application: &gtk::Application) {
    //let menu = gio::Menu::new();
    //let menu_bar = gio::Menu::new();
    //let more_menu = gio::Menu::new();
    //let switch_menu = gio::Menu::new();
    //let settings_menu = gio::Menu::new();
    //let submenu = gio::Menu::new();

    //// The first argument is the label of the menu item whereas the second is
    //// the action name. It'll makes more sense when you'll be reading the
    //// "add_actions" function.
    //menu.append(Some("Quit"), Some("app.quit"));

    //switch_menu.append(Some("Switch"), Some("app.switch"));
    //menu_bar.append_submenu(Some("_Switch"), &switch_menu);

    //settings_menu.append(Some("Sub another"), Some("app.sub_another"));
    //submenu.append(Some("Sub sub another"), Some("app.sub_sub_another"));
    //submenu.append(Some("Sub sub another2"), Some("app.sub_sub_another2"));
    //settings_menu.append_submenu(Some("Sub menu"), &submenu);
    //menu_bar.append_submenu(Some("_Another"), &settings_menu);

    //more_menu.append(Some("About"), Some("app.about"));
    //menu_bar.append_submenu(Some("?"), &more_menu);

    //application.set_app_menu(Some(&menu));
    //application.set_menubar(Some(&menu_bar));
}

/// This function creates "actions" which connect on the declared actions from
/// the menu items.
fn add_actions(
    application: &gtk::Application,
    window: &gtk::ApplicationWindow,
    rlr: Rc<Mutex<Rlr>>,
) {
    let freeze = gio::SimpleAction::new("freeze", None);
    let _rlr = rlr.clone();
    freeze.connect_activate(glib::clone!(@weak window => move |_, _| {
        {
            let mut lck = _rlr.lock().unwrap();
            lck.freeze = !lck.freeze;
        }
        window.queue_draw();
    }));

    let flip = gio::SimpleAction::new("flip", None);
    let _rlr = rlr.clone();
    flip.connect_activate(glib::clone!(@weak window => move |_, _| {
        {
            let mut lck = _rlr.lock().unwrap();
            if !lck.protractor {
                let _ = lck.rotate.next();
                let _ = lck.rotate.next();
            }
        }
        window.queue_draw();
    }));

    let rotate = gio::SimpleAction::new("rotate", None);
    let _rlr = rlr.clone();
    rotate.connect_activate(glib::clone!(@weak window => move |_, _| {
        {
            let mut lck = _rlr.lock().unwrap();
            if !lck.protractor {
                let tmp = lck.width;
                lck.width = lck.height;
                lck.height = tmp;
                lck.set_size(&window);
                if let Some(direction) = lck.rotate.next() {
                    let (mut x, mut y) = window.position();
                    let (height, width) = (lck.height, lck.width);
                    drop(lck);
                    if let Some(dir_x) = direction.0 {
                        if dir_x {
                            x += height;
                        } else {
                            x = x.saturating_sub(width);
                            x = std::cmp::max(10, x);
                        }
                    }
                    if let Some(dir_y) = direction.1 {
                        if dir_y {
                            y += width;
                        } else {
                            y = y.saturating_sub(height);
                            y = std::cmp::max(10, y);
                        }
                    }
                    window.move_(x, y);
                }
            }
        }
        window.queue_draw();
    }));

    let _rlr = rlr.clone();
    let protractor = gio::SimpleAction::new("protractor", None);
    protractor.connect_activate(glib::clone!(@weak window => move |_, _| {
        {
            let mut lck = _rlr.lock().unwrap();
            lck.protractor = !lck.protractor;
            if let Some((w, h)) = lck.p_dimens.take() {
                lck.p_dimens = Some((lck.width,lck.height ));
                lck.width = w;
                lck.height = h;
                window.resize(w, h);
            } else {
                lck.p_dimens = Some((lck.width,lck.height ));
                lck.set_size(&window);
            }

        }
        window.queue_draw();
    }));

    let quit = gio::SimpleAction::new("quit", None);
    quit.connect_activate(glib::clone!(@weak window => move |_, _| {
        window.close();
    }));

    let about = gio::SimpleAction::new("about", None);
    about.connect_activate(glib::clone!(@weak window => move |_, _| {
        let p = AboutDialog::new();
        p.set_program_name("rlr");
        p.set_logo(Some(&gtk::gdk_pixbuf::Pixbuf::from_xpm_data(
                    ICON,
        ).unwrap()));
        p.set_website_label(Some("https://github.com/epilys/rlr"));
        p.set_website(Some("https://github.com/epilys/rlr"));
        p.set_authors(&["Manos Pitsidianakis"]);
        p.set_copyright(Some("2021 - Manos Pitsidianakis"));
        p.set_title("About rlr");
        p.set_license_type(gtk::License::Gpl30);
        p.set_transient_for(Some(&window));
        p.set_comments(Some("- Quit with `q` or `Ctrl-Q`.
- Click to drag.
- Press `r` to rotate 90 degrees. Press `<Shift>r` to flip (mirror) the marks without rotation.
- Press `p` to toggle protractor mode.
- Press `f` or `<Space>` to toggle freezing the measurements.
- Press `Control_L` and drag the angle base side to rotate it in protractor mode.
- Press `Control_L` continuously to disable precision (measurements will snap to nearest integer).
- Press `+` to increase size.
- Press `-` to decrease size.
- Press `Up`, `Down`, `Left`, `Right` to move window position by 10 pixels. Also hold down `Control_L` to move by 1 pixel.
"));
        p.show_all();
    }));
    let settings = gio::SimpleAction::new("settings", None);

    settings.connect_activate(
        glib::clone!(@weak application, @weak window, @strong rlr => move |_, _| {
            let listbox = gtk::ListBox::builder()
                .visible(true)
                .sensitive(true)
                .can_focus(true)
                .expand(true)
                .build();
            let d = gtk::Dialog::builder()
                //.attached_to(&window)
                .application(&application)
                .title("rlr Settings")
                .has_focus(true)
                .can_focus(true)
                .sensitive(true)
                .border_width(15)
                .resizable(false)
                .transient_for(&window)
                .type_(gtk::WindowType::Toplevel)
                .type_hint(gdk::WindowTypeHint::Dialog)
                .build();
            // listbox.add(&gtk::Label::new(Some("Settings")));
            // d.content_area()
            let opacity_adj = gtk::Adjustment::new(0.0, 0.01, 1.0, 0.05, 0.1, 0.1);
            let font_size_adj = gtk::Adjustment::new(0.0, 0.1, 10.0, 0.05, 0.1, 0.1);
            let opacity_row = gtk::FlowBox::builder()
                .orientation(gtk::Orientation::Horizontal)
                .can_focus(true)
                .sensitive(true)
                .homogeneous(true)
                .expand(true)
                .visible(true)
                .max_children_per_line(2)
                .build();
            opacity_row.insert(&gtk::Label::new(Some("Opacity")), 0);
            opacity_row.insert(
                &gtk::Scale::builder()
                .can_focus(true)
                .sensitive(true)
                .visible(true)
                .digits(3)
                .adjustment(&opacity_adj)
                .expand(true)
                .build(),
                1,
            );
            let font_size_row = gtk::FlowBox::builder()
                .orientation(gtk::Orientation::Horizontal)
                .can_focus(true)
                .sensitive(true)
                .homogeneous(true)
                .expand(true)
                .visible(true)
                .max_children_per_line(2)
                .build();
            font_size_row.insert(&gtk::Label::new(Some("Font size factor")), 0);
            font_size_row.insert(
                &gtk::Scale::builder()
                .can_focus(true)
                .sensitive(true)
                .visible(true)
                .digits(3)
                .adjustment(&font_size_adj)
                .expand(true)
                .build(),
                1,
            );
            let primary_color_chooser = gtk::ColorButton::new();
            primary_color_chooser.set_expand(true);
            primary_color_chooser.set_use_alpha(true);
            let secondary_color_chooser = gtk::ColorButton::new();
            secondary_color_chooser.set_expand(true);
            secondary_color_chooser.set_use_alpha(true);
            {
                let lck = rlr.lock().unwrap();
                primary_color_chooser.set_rgba(&lck.settings.primary_color);
                secondary_color_chooser.set_rgba(&lck.settings.secondary_color);
                let gsettings_obj = lck.settings.obj.as_ref().unwrap().clone();
                drop(lck);
                gsettings_obj
                    .bind(Settings::WINDOW_OPACITY, &opacity_adj, "value")
                    .build();
                gsettings_obj
                    .bind(Settings::FONT_SIZE_FACTOR, &font_size_adj, "value")
                    .build();
                gsettings_obj
                    .bind(Settings::PRIMARY_COLOR, &primary_color_chooser, "rgba")
                    .mapping(|var, _| {
                        let hash: String = var.get()?;
                        let val: gdk::RGBA = gdk::RGBA::parse(&hash).ok()?;
                        Some(val.into())
                    })
                .set_mapping(|var, _| {
                    let val: gdk::RGBA = var.get().ok()?;
                    Some(val.to_str().to_string().into())
                })
                .build();
                gsettings_obj
                    .bind(Settings::SECONDARY_COLOR, &secondary_color_chooser, "rgba")
                    .mapping(|var, _| {
                        let hash: String = var.get()?;
                        let val: gdk::RGBA = gdk::RGBA::parse(&hash).ok()?;
                        Some(val.into())
                    })
                .set_mapping(|var, _| {
                    let val: gdk::RGBA = var.get().ok()?;
                    Some(val.to_str().to_string().into())
                })
                .build();
            }
            listbox.add(&opacity_row);
            listbox.add(&font_size_row);
            let primary_color_row = gtk::FlowBox::builder()
                .orientation(gtk::Orientation::Horizontal)
                .can_focus(true)
                .sensitive(true)
                .homogeneous(true)
                .expand(true)
                .visible(true)
                .max_children_per_line(2)
                .build();
            primary_color_row.insert(&gtk::Label::new(Some("Primary colour")), 0);
            primary_color_row.insert(&primary_color_chooser, 1);
            listbox.add(&primary_color_row);
            let secondary_color_row = gtk::FlowBox::builder()
                .orientation(gtk::Orientation::Horizontal)
                .can_focus(true)
                .sensitive(true)
                .homogeneous(true)
                .expand(true)
                .visible(true)
                .max_children_per_line(2)
                .build();
            secondary_color_row.insert(&gtk::Label::new(Some("Secondary colour")), 0);
            secondary_color_row.insert(&secondary_color_chooser, 1);
            listbox.add(&secondary_color_row);
            d.content_area().add(&listbox);
            d.content_area().set_visible(true);
            d.content_area().set_can_focus(true);
            d.add_button("Restore defaults", gtk::ResponseType::Reject);
            d.add_button("Close", gtk::ResponseType::Close);
            d.connect_response(
                glib::clone!(@weak window, @strong rlr => move |self_, response: gtk::ResponseType| {
                    match response {
                        gtk::ResponseType::Reject => {
                            let mut lck = rlr.lock().unwrap();
                            lck.settings = Settings {
                                obj: lck.settings.obj.take(),
                                changed_signal_id: lck.settings.changed_signal_id.take(),
                                ..Settings::default()
                            };
                            lck.settings.sync_write();
                            drop(lck);
                            window.queue_draw();
                        },
                        gtk::ResponseType::Close => self_.emit_close(),
                        _ => {},
                    }
                }),
            );

            d.show_all();
        }),
    );

    let _rlr = rlr.clone();
    let increase = gio::SimpleAction::new("increase", None);
    increase.connect_activate(glib::clone!(@weak window => move |_, _| {
        {
            let mut lck = _rlr.lock().unwrap();
            if !lck.protractor {
                if lck.rotate.is_rotated() {
                    lck.height += 50;
                } else {
                    lck.width += 50;
                }
            } else {
                lck.width += 50;
                lck.height = lck.width;
            }
            lck.set_size(&window);
        }
        window.queue_draw();
    }));
    let _rlr = rlr.clone();
    let decrease = gio::SimpleAction::new("decrease", None);
    decrease.connect_activate(glib::clone!(@weak window => move |_, _| {
        {
            let mut lck = _rlr.lock().unwrap();
            if !lck.protractor {
                if lck.rotate.is_rotated() {
                    lck.height -= 50;
                    lck.height = std::cmp::max(50, lck.height);
                } else {
                    lck.width -= 50;
                    lck.width = std::cmp::max(50, lck.width);
                }
            } else {
                lck.width -= 50;
                lck.width = std::cmp::max(50, lck.width);
                lck.height = lck.width;
            }
            lck.set_size(&window);
        }
        window.queue_draw();
    }));
    let _rlr = rlr.clone();
    let increase_font_size = gio::SimpleAction::new("increase_font_size", None);
    increase_font_size.connect_activate(glib::clone!(@weak window => move |_, _| {
        {
            let mut lck = _rlr.lock().unwrap();
            lck.settings.font_size_factor += 0.05;
            lck.settings.font_size_factor = lck.settings.font_size_factor.clamp(0.1, 10.0);
            lck.settings.sync_write();
        }
        window.queue_draw();
    }));
    let _rlr = rlr.clone();
    let decrease_font_size = gio::SimpleAction::new("decrease_font_size", None);
    decrease_font_size.connect_activate(glib::clone!(@weak window => move |_, _| {
        {
            let mut lck = _rlr.lock().unwrap();
            lck.settings.font_size_factor -= 0.05;
            lck.settings.font_size_factor = lck.settings.font_size_factor.clamp(0.1, 10.0);
            lck.settings.sync_write();
        }
        window.queue_draw();
    }));
    let _rlr = rlr.clone();
    let move_right = gio::SimpleAction::new("move_right", None);
    move_right.connect_activate(glib::clone!(@weak window => move |_, _| {
        let rlr = _rlr.clone();
        let precision = rlr.lock().unwrap().precision;
        let (mut x, y) = window.position();
        if !precision {
            x += 1;
        } else {
            x += 10;
        }
        window.move_(x, y);
        window.queue_draw();
    }));

    let _rlr = rlr.clone();
    let move_left = gio::SimpleAction::new("move_left", None);
    move_left.connect_activate(glib::clone!(@weak window => move |_, _| {
        let rlr = _rlr.clone();
        let precision = rlr.lock().unwrap().precision;
        let (mut x, y) = window.position();
        if !precision {
            x -= 1;
        } else {
            x -= 10;
        }
        window.move_(x, y);
        window.queue_draw();
    }));

    let _rlr = rlr.clone();
    let move_up = gio::SimpleAction::new("move_up", None);
    move_up.connect_activate(glib::clone!(@weak window => move |_, _| {
        let rlr = _rlr.clone();
        let precision = rlr.lock().unwrap().precision;
        let (x, mut y) = window.position();
        if !precision {
            y -= 1;
        } else {
            y -= 10;
        }
        window.move_(x, y);
        window.queue_draw();
    }));

    let move_down = gio::SimpleAction::new("move_down", None);
    move_down.connect_activate(glib::clone!(@weak window => move |_, _| {
        let precision = rlr.lock().unwrap().precision;
        let (x, mut y) = window.position();
        if !precision {
            y += 1;
        } else {
            y += 10;
        }
        window.move_(x, y);
        window.queue_draw();
    }));

    // We need to add all the actions to the application so they can be taken into
    // account.

    application.add_action(&move_right);
    application.add_action(&move_left);
    application.add_action(&move_up);
    application.add_action(&move_down);
    application.add_action(&increase);
    application.add_action(&decrease);
    application.add_action(&increase_font_size);
    application.add_action(&decrease_font_size);
    application.add_action(&freeze);
    application.add_action(&protractor);
    application.add_action(&rotate);
    application.add_action(&flip);
    application.add_action(&about);
    application.add_action(&settings);
    application.add_action(&quit);
}
