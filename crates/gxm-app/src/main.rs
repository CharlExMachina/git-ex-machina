fn main() {
    // What Wayland and X11 report the window as, so the desktop matches it to the AppImage's
    // `git-ex-machina.desktop` and its icon.
    rune::set_app_id(gxm_app::APP_ID);
    gxm_app::app().run();
}
