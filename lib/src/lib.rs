#[derive(Default, Debug)]
pub struct State {
    pub counter: usize,
    pub name: String,
}

#[no_mangle]
pub fn step(state: &mut State) {
    state.counter += 1;
    state.name = "bla".to_string(); //.push_str("a");
}
