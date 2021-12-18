/*
 * rlr
 *
 * Copyright 2021 - Manos Pitsidianakis
 *
 * This file is part of rlr.
 *
 * rlr is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * rlr is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with rlr. If not, see <http://www.gnu.org/licenses/>.
 */

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use gtk::{AboutDialog, DrawingArea};
use std::f64::consts::{FRAC_PI_2, PI};
use std::sync::{Arc, Mutex};

use gtk::cairo::{Context, FontSlant, FontWeight};

#[derive(Debug)]
struct Rlr {
    position: (f64, f64),
    root_position: (i32, i32),
    width: i32,
    height: i32,
    p_dimens: Option<(i32, i32)>,
    freeze: bool,
    rotate: bool,
    protractor: bool,
    precision: bool,
    edit_angle_offset: bool,
    angle_offset: f64,
}

impl Default for Rlr {
    fn default() -> Self {
        Rlr {
            position: (0., 0.),
            root_position: (0, 0),
            width: 500,
            height: 35,
            p_dimens: None,
            freeze: false,
            rotate: false,
            protractor: false,
            precision: true,
            edit_angle_offset: false,
            angle_offset: 0.,
        }
    }
}

fn draw_rlr(rlr: Arc<Mutex<Rlr>>, drar: &DrawingArea, cr: &Context) -> Inhibit {
    let lck = rlr.lock().unwrap();
    if lck.protractor {
        return lck.draw_douglas(drar, cr);
    }
    lck.draw_rlr(drar, cr)
}

impl Rlr {
    fn set_size(&mut self, window: &gtk::ApplicationWindow) {
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

    fn draw_douglas(&self, _drar: &DrawingArea, cr: &Context) -> Inhibit {
        let length: f64 = std::cmp::min(self.width, self.height) as f64;
        let root_position = self.root_position;
        let root_position = (
            root_position.0 as f64 - length / 2.,
            -1. * (root_position.1 as f64 - length / 2.),
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
        cr.set_source_rgb(1., 1.0, 1.0);
        cr.fill().expect("Invalid cairo surface state");

        let _pixels_per_tick = 10;
        let tick_size = 5.;
        cr.set_source_rgb(0.1, 0.1, 0.1);
        cr.set_line_width(1.);

        //cr.rectangle(0.5, 0.5, length - 1.0, length - 1.0);
        //cr.stroke().expect("Invalid cairo surface state");

        for i in 1..(length / 2.).floor() as i64 {
            let r = (i as f64) * tick_size * 10.;
            cr.arc(length / 2., length / 2., r, 0., 2. * std::f64::consts::PI);
            cr.stroke().expect("Invalid cairo surface state");
            if 2. * r >= length {
                break;
            }
        }

        let mut a = 0.;
        while a <= (2. * PI) {
            let tick_size = if (a.abs() * (180. / PI)) % 30. <= 0.55 {
                5.0 * tick_size
            } else if (a.abs() * (180. / PI)) % 5. <= 0.5 {
                1.5 * tick_size
            } else {
                tick_size
            };
            cr.save().unwrap();
            cr.move_to(length / 2. - 0.5, length / 2. - 0.5);
            cr.rotate(2. * PI - a - FRAC_PI_2);
            let cur = cr.current_point().unwrap();
            cr.move_to(cur.0 + length / 2. - 0.5 - tick_size, cur.1);
            cr.line_to(cur.0 + length / 2. - 0.5, cur.1); //.+(xr*xr+yr*yr).sqrt());
            cr.stroke().expect("Invalid cairo surface state");
            cr.restore().unwrap();
            a += 0.01;
        }

        cr.save().unwrap();
        cr.set_line_width(2.);
        cr.move_to(length / 2. - 0.5, length / 2. - 0.5);
        cr.rotate(2. * PI - FRAC_PI_2 - self.angle_offset);
        let cur = cr.current_point().unwrap();
        cr.line_to(cur.0, cur.1 + length / 2. - 0.5); //.+(xr*xr+yr*yr).sqrt());
        cr.stroke().expect("Invalid cairo surface state");
        cr.restore().unwrap();

        cr.save().unwrap();
        let _angle = if self.precision {
            angle + FRAC_PI_2
        } else {
            angle.round() + FRAC_PI_2
        };
        cr.move_to(length / 2. - 0.5, length / 2. - 0.5);
        cr.rotate(2. * PI - _angle);
        let cur = cr.current_point().unwrap();
        cr.arc(cur.0, cur.1, 2., 0., 2. * std::f64::consts::PI);
        cr.stroke().expect("Invalid cairo surface state");
        cr.move_to(cur.0, cur.1);
        cr.line_to(cur.0, cur.1 + length / 2. - 0.5); //.+(xr*xr+yr*yr).sqrt());
        cr.stroke().expect("Invalid cairo surface state");
        cr.restore().unwrap();
        cr.select_font_face("Sans", FontSlant::Normal, FontWeight::Normal);
        //cr.set_font_size(0.35);

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
        cr.move_to(length / 2. - 5.5, length / 2. - 15.5);
        cr.show_text(&format!(
            " {:.2}rad {:.2}°",
            if self.precision { angle } else { angle.round() },
            if self.precision { angle } else { angle.round() } * (180. / PI)
        ))
        .expect("Invalid cairo surface state");

        Inhibit(false)
    }

    fn draw_rlr(&self, _drar: &DrawingArea, cr: &Context) -> Inhibit {
        let position = self.position;
        /*
        let root_window = drar
            .display()
            .device_manager()
            .unwrap()
            .client_pointer()
            .unwrap()
            .position();
        std::dbg!(root_window);
        */
        let length: f64 = self.width as f64;
        let height: f64 = self.height as f64;
        let breadth = if self.rotate {
            self.width as f64
        } else {
            self.height as f64
        };

        //println!("Extents: {:?}", cr.fill_extents());

        //cr.scale(500f64, 40f64);

        //cr.set_source_rgb(250.0 / 255.0, 224.0 / 255.0, 55.0 / 255.0);
        cr.set_source_rgb(1., 1.0, 1.0);
        cr.paint().expect("Invalid cairo surface state");

        let _pixels_per_tick = 10;
        let tick_size = 5.;
        let mut i = 0;
        let mut x: f64;
        cr.set_source_rgb(0.1, 0.1, 0.1);
        cr.set_line_width(1.);
        if self.rotate {
            while i < self.height {
                x = (i as f64).floor() + 0.5;
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
                    let label = format!("{}", i);
                    let extents = cr
                        .text_extents(&label)
                        .expect("Invalid cairo surface state");
                    cr.move_to(breadth / 2. - 2.5 - extents.width as f64 / 2., x);
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

            cr.select_font_face("Sans", FontSlant::Normal, FontWeight::Normal);
            //cr.set_font_size(0.35);

            cr.move_to(breadth / 4., x);
            cr.show_text(&format!("{}px", pos))
                .expect("Invalid cairo surface state");

            cr.rectangle(0.5, 0.5, length - 1.0, height - 1.0);
            cr.stroke().expect("Invalid cairo surface state");
        } else {
            while i < self.width {
                x = (i as f64).floor() + 0.5;
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
                    let label = format!("{}", i);
                    let extents = cr
                        .text_extents(&label)
                        .expect("Invalid cairo surface state");
                    cr.move_to(x - extents.width as f64 / 2., breadth / 2. + 2.5);
                    cr.show_text(&label).expect("Invalid cairo surface state");
                }
                i += 2;
            }
            let pos = if self.precision {
                position.0.floor()
            } else {
                (position.0 / 10.).floor() * 10.
            };
            let x = pos + 0.5;
            cr.move_to(x, 1.0);
            cr.line_to(x, breadth);
            cr.stroke().expect("Invalid cairo surface state");

            cr.select_font_face("Sans", FontSlant::Normal, FontWeight::Normal);
            //cr.set_font_size(0.35);

            cr.move_to(x, breadth / 2.);
            cr.show_text(&format!("{}px", pos))
                .expect("Invalid cairo surface state");

            cr.rectangle(0.5, 0.5, length - 1.0, breadth - 1.0);
            cr.stroke().expect("Invalid cairo surface state");
        }

        Inhibit(false)
    }
}

fn main() {
    let application = gtk::Application::new(
        Some("com.github.gtk-rs.examples.cairotest"),
        Default::default(),
    );

    let rlr = Arc::new(Mutex::new(Rlr::default()));

    application.connect_startup(|application: &gtk::Application| {
        application.set_accels_for_action("app.quit", &["<Primary>Q", "Q"]);
        application.set_accels_for_action("app.rotate", &["R"]);
        application.set_accels_for_action("app.protractor", &["P"]);
        application.set_accels_for_action("app.freeze", &["F", "space"]);
        application.set_accels_for_action("app.increase", &["plus"]);
        application.set_accels_for_action("app.decrease", &["minus"]);
        application.set_accels_for_action("app.about", &["question", "F1"]);
        application.set_accels_for_action("app.move_right", &["Right", "<Primary>Right"]);
        application.set_accels_for_action("app.move_left", &["Left", "<Primary>Left"]);
        application.set_accels_for_action("app.move_up", &["Up", "<Primary>Up"]);
        application.set_accels_for_action("app.move_down", &["Down", "<Primary>Down"]);
        application.set_accels_for_action("app.move_to_center", &["Home"]);
    });
    application.connect_activate(move |application: &gtk::Application| {
        let _rlr = rlr.clone();
        let _rlr2 = rlr.clone();
        drawable(
            application,
            _rlr,
            move |drar: &DrawingArea, cr: &Context| -> Inhibit {
                let _rlr = _rlr2.clone();
                draw_rlr(_rlr, drar, cr)
            },
        );
    });

    application.run();
}

fn drawable<F>(application: &gtk::Application, rlr: Arc<Mutex<Rlr>>, draw_fn: F)
where
    F: Fn(&DrawingArea, &Context) -> Inhibit + 'static,
{
    let window = gtk::ApplicationWindow::builder()
        .application(application)
        .events(gdk::EventMask::POINTER_MOTION_MASK)
        .build();

    set_visual(&window, None);

    window.connect_screen_changed(set_visual);
    {
        let rlr = rlr.clone();
        let window = window.clone();
        let tick = move || {
            let mut lck = rlr.lock().unwrap();
            if lck.edit_angle_offset || lck.freeze {
                return glib::Continue(true);
            }
            if let Some(screen) = window.window() {
                let root_origin = screen.root_origin();
                let display = screen.display();
                let (_, x, y) = display
                    .device_manager()
                    .unwrap()
                    .client_pointer()
                    .unwrap()
                    .position();
                let root_position = (x - root_origin.0, y - root_origin.1);

                if root_position != lck.root_position {
                    if lck.protractor {
                        lck.root_position = root_position;
                        lck.position.0 = root_position.0 as f64;
                        lck.position.1 = root_position.1 as f64;
                        window.queue_draw();
                    } else if lck.rotate && root_position.1 < lck.width && root_position.1 > 0 {
                        lck.root_position = root_position;
                        lck.position.1 = root_position.1 as f64;
                        window.queue_draw();
                    } else if !lck.rotate && root_position.0 < lck.width && root_position.0 > 0 {
                        lck.root_position = root_position;
                        lck.position.0 = root_position.0 as f64;
                        window.queue_draw();
                    }
                }
            }
            // we could return glib::Continue(false) to stop our clock after this tick
            glib::Continue(true)
        };

        // executes the closure once every second
        glib::timeout_add_local(std::time::Duration::from_millis(10), tick);
    }

    window.connect_enter_notify_event(enter_notify);
    window.connect_leave_notify_event(leave_notify);
    let _rlr = rlr.clone();
    window.connect_button_press_event(
        move |window: &gtk::ApplicationWindow, ev: &gtk::gdk::EventButton| -> Inhibit {
            let rlr = _rlr.clone();
            let mut lck = rlr.lock().unwrap();
            //println!("drag begin");

            if ev.button() == 1 && !lck.precision {
                lck.edit_angle_offset = true;
            } else {
                window.begin_move_drag(
                    ev.button() as _,
                    ev.root().0 as _,
                    ev.root().1 as _,
                    ev.time(),
                );
            }
            Inhibit(false)
        },
    );
    let _rlr = rlr.clone();
    window.connect_button_release_event(
        move |_application: &gtk::ApplicationWindow, ev: &gtk::gdk::EventButton| -> Inhibit {
            let rlr = _rlr.clone();
            let mut lck = rlr.lock().unwrap();
            //println!("drag end");
            if ev.button() == 1 {
                lck.edit_angle_offset = false;
            }
            Inhibit(false)
        },
    );
    let _rlr = rlr.clone();
    window.connect_key_press_event(
        move |window: &gtk::ApplicationWindow, ev: &gtk::gdk::EventKey| -> Inhibit {
            //println!("press {}", ev.keyval().name().unwrap().as_str());
            if ev
                .keyval()
                .name()
                .map(|n| n.as_str() == "Control_L")
                .unwrap_or(false)
            {
                let rlr = _rlr.clone();
                let mut lck = rlr.lock().unwrap();
                lck.precision = false;
                window.queue_draw();
            }
            Inhibit(false)
        },
    );
    let _rlr = rlr.clone();
    window.connect_key_release_event(
        move |window: &gtk::ApplicationWindow, ev: &gtk::gdk::EventKey| -> Inhibit {
            //println!("release {}", ev.keyval().name().unwrap().as_str());
            if ev
                .keyval()
                .name()
                .map(|n| n.as_str() == "Control_L")
                .unwrap_or(false)
            {
                let rlr = _rlr.clone();
                let mut lck = rlr.lock().unwrap();
                lck.precision = true;
                window.queue_draw();
            }
            Inhibit(false)
        },
    );
    let _rlr = rlr.clone();
    window.connect_motion_notify_event(
        move |window: &gtk::ApplicationWindow, motion: &gdk::EventMotion| -> Inhibit {
            let rlr = _rlr.clone();
            let mut lck = rlr.lock().unwrap();
            lck.position = motion.position();
            if lck.edit_angle_offset {
                let (xr, yr) = lck.position;
                let translated_position = (xr - lck.width as f64 / 2., lck.width as f64 / 2. - yr);
                let angle = lck.calc_angle_of_point(translated_position);
                lck.angle_offset = angle; // + FRAC_PI_2;
            }
            window.queue_draw();
            Inhibit(false)
        },
    );
    let _rlr = rlr.clone();
    window.connect_configure_event(
        move |window: &gtk::ApplicationWindow, event: &gdk::EventConfigure| -> bool {
            let rlr = _rlr.clone();
            let mut lck = rlr.lock().unwrap();
            lck.width = event.size().0 as i32;
            lck.height = event.size().1 as i32;
            window.queue_draw();

            false
        },
    );
    window.set_app_paintable(true); // crucial for transparency
    window.set_resizable(true);
    window.set_decorated(false);
    //#[cfg(debug_assertions)]
    //gtk::Window::set_interactive_debugging(true);

    let drawing_area = Box::new(DrawingArea::new)();

    drawing_area.connect_draw(draw_fn);

    if let Ok(lck) = rlr.lock() {
        window.set_default_size(lck.width, lck.height);
    }

    window.add(&drawing_area);
    window.set_opacity(0.8);

    build_system_menu(application);

    add_actions(application, &window, rlr);

    window.show_all();
}

fn enter_notify(window: &gtk::ApplicationWindow, _crossing: &gtk::gdk::EventCrossing) -> Inhibit {
    //println!("enter");
    if let Some(screen) = window.window() {
        let display = screen.display();
        display.beep();
        if let Some(gdk_window) = window.window() {
            gdk_window.set_cursor(Some(
                &gtk::gdk::Cursor::from_name(&display, "move").unwrap(),
            ));
        }
    }
    Inhibit(false)
}

fn leave_notify(
    _application: &gtk::ApplicationWindow,
    _crossing: &gtk::gdk::EventCrossing,
) -> Inhibit {
    //println!("leave");
    Inhibit(false)
}

fn set_visual(window: &gtk::ApplicationWindow, _screen: Option<&gtk::gdk::Screen>) {
    if let Some(screen) = window.screen() {
        if let Some(ref visual) = screen.rgba_visual() {
            window.set_visual(Some(visual)); // crucial for transparency
        }
    }
}

fn build_system_menu(_application: &gtk::Application) {
    //let menu = gio::Menu::new();
    //let menu_bar = gio::Menu::new();
    //let more_menu = gio::Menu::new();
    //let switch_menu = gio::Menu::new();
    //let settings_menu = gio::Menu::new();
    //let submenu = gio::Menu::new();

    //// The first argument is the label of the menu item whereas the second is the action name. It'll
    //// makes more sense when you'll be reading the "add_actions" function.
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

/// This function creates "actions" which connect on the declared actions from the menu items.
fn add_actions(
    application: &gtk::Application,
    window: &gtk::ApplicationWindow,
    rlr: Arc<Mutex<Rlr>>,
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

    let rotate = gio::SimpleAction::new("rotate", None);
    let _rlr = rlr.clone();
    rotate.connect_activate(glib::clone!(@weak window => move |_, _| {
        {
            let mut lck = _rlr.lock().unwrap();
            if !lck.protractor {
            lck.rotate = !lck.rotate;
            let tmp = lck.width;
            lck.width = lck.height;
            lck.height = tmp;
            lck.set_size(&window);
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
        p.set_website_label(Some("https://github.com/epilys/rlr"));
        p.set_website(Some("https://github.com/epilys/rlr"));
        p.set_authors(&["Manos Pitsidianakis"]);
        p.set_copyright(Some("2021 - Manos Pitsidianakis"));
        p.set_title("About rlr");
        p.set_license_type(gtk::License::Gpl30);
        p.set_transient_for(Some(&window));
        p.set_comments(Some("- Quit with `q` or `Ctrl-Q`.
- Click to drag.
- Press `r` to rotate 90 degrees.
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

    let _rlr = rlr.clone();
    let increase = gio::SimpleAction::new("increase", None);
    increase.connect_activate(glib::clone!(@weak window => move |_, _| {
        {
            let mut lck = _rlr.lock().unwrap();
            if !lck.protractor {
                if lck.rotate {
                    lck.height += 50;
                } else {
                    lck.width += 50;
                }
                lck.set_size(&window);
            } else {
                lck.width += 50;
                lck.height = lck.width;
                lck.set_size(&window);
            }
        }
        window.queue_draw();
    }));
    let _rlr = rlr.clone();
    let decrease = gio::SimpleAction::new("decrease", None);
    decrease.connect_activate(glib::clone!(@weak window => move |_, _| {
        {
            let mut lck = _rlr.lock().unwrap();
            if !lck.protractor {
                if lck.rotate {
                    lck.height -= 50;
                    lck.height = std::cmp::max(50, lck.height);
                } else {
                    lck.width -= 50;
                    lck.width = std::cmp::max(50, lck.width);
                }
                lck.set_size(&window);
            } else {
                lck.width -= 50;
                lck.width = std::cmp::max(50, lck.width);
                lck.height = lck.width;
                lck.set_size(&window);
            }
        }
        window.queue_draw();
    }));
    let _rlr = rlr.clone();
    let move_right = gio::SimpleAction::new("move_right", None);
    move_right.connect_activate(glib::clone!(@weak window => move |_, _| {
        let rlr = _rlr.clone();
        let lck = rlr.lock().unwrap();
        let (mut x, y) = window.position();
        if !lck.precision {
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
        let lck = rlr.lock().unwrap();
        let (mut x, y) = window.position();
        if !lck.precision {
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
        let lck = rlr.lock().unwrap();
        let (x, mut y) = window.position();
        if !lck.precision {
            y -= 1;
        } else {
            y -= 10;
        }
        window.move_(x, y);
        window.queue_draw();
    }));
    let _rlr = rlr.clone();
    let move_down = gio::SimpleAction::new("move_down", None);
    move_down.connect_activate(glib::clone!(@weak window => move |_, _| {
        let rlr = _rlr.clone();
        let lck = rlr.lock().unwrap();
        let (x, mut y) = window.position();
        if !lck.precision {
            y += 1;
        } else {
            y += 10;
        }
        window.move_(x, y);
        window.queue_draw();
    }));
    // We need to add all the actions to the application so they can be taken into account.
    application.add_action(&move_right);
    application.add_action(&move_left);
    application.add_action(&move_up);
    application.add_action(&move_down);
    application.add_action(&increase);
    application.add_action(&decrease);
    application.add_action(&freeze);
    application.add_action(&protractor);
    application.add_action(&rotate);
    application.add_action(&about);
    application.add_action(&quit);
}
