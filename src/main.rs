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
use std::sync::{Arc, Mutex};

use gtk::cairo::{Context, FontSlant, FontWeight};

#[derive(Debug)]
struct Rlr {
    position: (f64, f64),
    root_position: (i32, i32),
    breadth: f64,
    width: i32,
    height: i32,
}

impl Default for Rlr {
    fn default() -> Self {
        Rlr {
            position: (0., 0.),
            root_position: (0, 0),
            breadth: 35.,
            width: 500,
            height: 35,
        }
    }
}

fn draw_rlr(rlr: Arc<Mutex<Rlr>>, drar: &DrawingArea, cr: &Context) -> Inhibit {
    let lck = rlr.lock().unwrap();
    let position = lck.position;
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
    let length: f64 = drar.allocated_width() as f64;
    let _height: f64 = drar.allocated_height() as f64;

    //println!("Extents: {:?}", cr.fill_extents());

    //cr.scale(500f64, 40f64);

    //cr.set_source_rgb(250.0 / 255.0, 224.0 / 255.0, 55.0 / 255.0);
    cr.set_source_rgb(1., 1.0, 1.0);
    cr.paint().expect("Invalid cairo surface state");

    let _pixels_per_tick = 10;
    let tick_size = 5.;
    let mut i = 0;
    let mut x: f64;
    let breadth = lck.breadth;
    cr.set_source_rgb(0.1, 0.1, 0.1);
    cr.set_line_width(1.);
    while i < lck.width {
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
        i += 2;
    }

    let x = position.0.floor() + 0.5;
    cr.move_to(x, 1.0);
    cr.line_to(x, breadth);
    cr.stroke().expect("Invalid cairo surface state");

    cr.select_font_face("Sans", FontSlant::Normal, FontWeight::Normal);
    //cr.set_font_size(0.35);

    cr.move_to(x, breadth / 2.);
    cr.show_text(&format!("{}px", position.0.floor()))
        .expect("Invalid cairo surface state");

    cr.rectangle(0.5, 0.5, length - 1.0, breadth - 1.0);
    cr.stroke().expect("Invalid cairo surface state");
    Inhibit(false)
}

fn main() {
    let application = gtk::Application::new(
        Some("com.github.gtk-rs.examples.cairotest"),
        Default::default(),
    );

    let rlr = Arc::new(Mutex::new(Rlr::default()));

    application.connect_startup(|application: &gtk::Application| {
        application.set_accels_for_action("app.quit", &["<Primary>Q"]);
        application.set_accels_for_action("app.quit", &["Q"]);
        //application.set_accels_for_action("app.about", &["<Primary>A"]);
    });
    application.connect_activate(move |application: &gtk::Application| {
        let _rlr = rlr.clone();
        let _rlr2 = rlr.clone();
        let (width, height) = {
            let l = _rlr.lock().unwrap();
            (l.width, l.height)
        };
        drawable(
            application,
            _rlr,
            width,
            height,
            move |drar: &DrawingArea, cr: &Context| -> Inhibit {
                let _rlr = _rlr2.clone();
                draw_rlr(_rlr, drar, cr)
            },
        );
    });

    application.run();
}

fn drawable<F>(
    application: &gtk::Application,
    rlr: Arc<Mutex<Rlr>>,
    width: i32,
    height: i32,
    draw_fn: F,
) where
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
                if root_position != lck.root_position
                    && root_position.0 < lck.width
                    && root_position.0 > 0
                {
                    lck.root_position = root_position;
                    lck.position.0 = root_position.0 as f64;
                    window.queue_draw();
                }
            }
            // we could return glib::Continue(false) to stop our clock after this tick
            glib::Continue(true)
        };

        // executes the closure once every second
        glib::timeout_add_local(std::time::Duration::from_millis(80), tick);
    }

    window.connect_enter_notify_event(enter_notify);
    window.connect_leave_notify_event(leave_notify);
    //let _rlr = rlr.clone();
    window.connect_button_press_event(
        move |window: &gtk::ApplicationWindow, ev: &gtk::gdk::EventButton| -> Inhibit {
            //let rlr = _rlr.clone();
            //println!("drag begin");
            window.begin_move_drag(
                ev.button() as _,
                ev.root().0 as _,
                ev.root().1 as _,
                ev.time(),
            );
            Inhibit(false)
        },
    );
    let _rlr = rlr.clone();
    window.connect_button_release_event(
        move |_application: &gtk::ApplicationWindow, _ev: &gtk::gdk::EventButton| -> Inhibit {
            //let rlr = _rlr.clone();
            //println!("drag end");
            Inhibit(false)
        },
    );
    window.connect_motion_notify_event(
        move |window: &gtk::ApplicationWindow, motion: &gdk::EventMotion| -> Inhibit {
            let rlr = rlr.clone();
            let mut lck = rlr.lock().unwrap();
            lck.position = motion.position();
            window.queue_draw();
            Inhibit(false)
        },
    );
    window.set_app_paintable(true); // crucial for transparency
                                    //window.set_resizable(true);
    window.set_decorated(false);
    #[cfg(debug_assertions)]
    gtk::Window::set_interactive_debugging(true);

    let drawing_area = Box::new(DrawingArea::new)();

    drawing_area.connect_draw(draw_fn);

    window.set_default_size(width, height);

    window.add(&drawing_area);
    window.set_opacity(0.8);

    build_system_menu(application);

    add_actions(application, &window);

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
fn add_actions(application: &gtk::Application, window: &gtk::ApplicationWindow) {
    let quit = gio::SimpleAction::new("quit", None);
    quit.connect_activate(glib::clone!(@weak window => move |_, _| {
        window.close();
    }));

    let about = gio::SimpleAction::new("about", None);
    about.connect_activate(glib::clone!(@weak window => move |_, _| {
        let p = AboutDialog::new();
        p.set_website_label(Some("gtk-rs"));
        p.set_website(Some("http://gtk-rs.org"));
        p.set_authors(&["gtk-rs developers"]);
        p.set_title("About!");
        p.set_transient_for(Some(&window));
        p.show_all();
    }));

    // We need to add all the actions to the application so they can be taken into account.
    application.add_action(&about);
    application.add_action(&quit);
}
