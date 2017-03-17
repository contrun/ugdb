use unsegen::{
    SeparatingStyle,
    VerticalLayout,
    Widget,
    Window,
    WriteBehavior,
};
use input::{
    InputEvent,
};
use syntect::highlighting::{
    Theme,
};

use gdbmi;
use gdbmi::output::{
    OutOfBandRecord,
    AsyncKind,
    AsyncClass,
    NamedValues,
};

use super::console::Console;
use super::srcview::SrcView;
use super::pseudoterminal::PseudoTerminal;

pub struct Tui<'a> {
    console: Console,
    process_pty: PseudoTerminal,
    src_view: SrcView<'a>,

    left_layout: VerticalLayout,
    right_layout: VerticalLayout,
}

impl<'a> Tui<'a> {

    pub fn new(process_pty: ::pty::PTYInput, highlighting_theme: &'a Theme) -> Self {
        Tui {
            console: Console::new(),
            process_pty: PseudoTerminal::new(process_pty),
            src_view: SrcView::new(highlighting_theme),
            left_layout: VerticalLayout::new(SeparatingStyle::Draw('=')),
            right_layout: VerticalLayout::new(SeparatingStyle::Draw('=')),
        }
    }

    fn handle_async_record(&mut self, kind: AsyncKind, class: AsyncClass, mut results: NamedValues) {
        match (kind, class) {
            (AsyncKind::Exec, AsyncClass::Stopped) => {
                self.console.add_debug_message(format!("stopped: {:?}", results));
                if let Some(frame_object) = results.remove("frame") {
                    let mut frame = frame_object.unwrap_tuple_or_named_value_list();
                    if let Some(path_object) = frame.remove("fullname") { // File information may not be present
                        let path = path_object.unwrap_const();
                        let line = frame.remove("line").expect("line present").unwrap_const().parse::<usize>().expect("parse usize") - 1; //TODO we probably want to treat the conversion line_number => buffer index somewhere else...
                        let _ = self.src_view.show_in_file_viewer(path, line); // GDB may give out invalid paths, so we just ignore them (at least for now)
                    }
                }
            },
            (kind, class) => self.console.add_debug_message(format!("unhandled async_record: [{:?}, {:?}] {:?}", kind, class, results)),
        }
    }

    pub fn add_out_of_band_record(&mut self, record: OutOfBandRecord) {
        match record {
            OutOfBandRecord::StreamRecord{ kind: _, data} => {
                self.console.add_message(data);
            },
            OutOfBandRecord::AsyncRecord{token: _, kind, class, results} => {
                self.handle_async_record(kind, class, results);
            },

        }
    }

    pub fn add_pty_input(&mut self, input: Vec<u8>) {
        self.process_pty.add_byte_input(input);
    }

    pub fn add_debug_message(&mut self, msg: &str) {
        self.console.add_debug_message(format!("Debug: {}", msg));
    }

    pub fn draw(&mut self, window: Window) {
        use unsegen::{TextAttribute, Color, Style};
        let split_pos = window.get_width()/2-1;
        let (window_l, rest) = window.split_h(split_pos);

        let (mut separator, window_r) = rest.split_h(2);

        separator.set_default_format(TextAttribute::new(Color::green(), Color::blue(), Style::new().bold().italic().underline()));
        separator.fill('|');

        let mut left_widgets: Vec<&mut Widget> = vec![&mut self.src_view, &mut self.console];
        self.left_layout.draw(window_l, &mut left_widgets);

        let mut right_widgets: Vec<&mut Widget> = vec![&mut self.process_pty];
        self.right_layout.draw(window_r, &mut right_widgets);
    }

    pub fn event(&mut self, event: InputEvent, gdb: &mut gdbmi::GDB) { //TODO more console events
        match event {
            InputEvent::ConsoleEvent(event) => {
                self.console.event(event, gdb);
            },
            InputEvent::PseudoTerminalEvent(event) => {
                event.chain(WriteBehavior::new(&mut self.process_pty));
            },
            InputEvent::SourcePagerEvent(event) => {
                self.src_view.event(event, gdb)
            },
            InputEvent::Quit => {
                unreachable!("quit should have been caught in main" )
            }, //TODO this is ugly
        }
    }
}
