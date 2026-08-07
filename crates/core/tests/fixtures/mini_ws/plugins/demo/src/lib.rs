// fixture: healthy-ish plugin surface for agal scan tests
use lx_slint_editor::something;

pub struct DemoParams {
    pub gain: f32,
    pub bypass: bool,
}

pub struct Demo;

impl PluginLogic for Demo {
    fn process(&mut self) {}
}

fn editor() {}
