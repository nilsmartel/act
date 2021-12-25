use lazy_static::lazy_static;
use std::any::Any;
use std::sync::{Mutex, MutexGuard, RwLock};

#[cfg(test)]
mod tests {
    #[test]
    fn recover_state() {
        let mut hooks = super::Hooks::default();
        let (_, _) = hooks.use_state("what");
        let (_, _) = hooks.use_state(123);
        let (_, _) = hooks.use_state(3.145);
        let (_, _) = hooks.use_state(true);

        let mut hooks = super::Hooks::default();

        let (a, _) = hooks.use_state("no");
        let (b, _) = hooks.use_state(1231);
        let (c, _) = hooks.use_state(3.14325);
        let (d, _) = hooks.use_state(false);

        assert_eq!(a, "what");
        assert_eq!(b, 123);
        assert_eq!(c, 3.145);
        assert_eq!(d, true);
    }

    #[test]
    fn set_state() {
        let mut hooks = super::Hooks::default();
        let (_, set_a) = hooks.use_state("what");
        let (_, set_b) = hooks.use_state(123);
        let (_, set_c) = hooks.use_state(3.145);
        let (_, set_d) = hooks.use_state(true);
        set_a("möp");
        set_b(314);
        set_c(0.0);
        set_d(false);

        let mut hooks = super::Hooks::default();

        let (a, _) = hooks.use_state("what");
        let (b, _) = hooks.use_state(123);
        let (c, _) = hooks.use_state(3.145);
        let (d, _) = hooks.use_state(true);

        assert_eq!(a, "möp");
        assert_eq!(b, 314);
        assert_eq!(c, 0.0);
        assert_eq!(d, false);
    }
}

lazy_static! {
    static ref STATE_TREE: Mutex<StateTree> = Mutex::new(StateTree::default());
}

#[derive(Default)]
struct StateTree {
    state: State,

    /// When some component uses state, it's subcomponents will get their own state, which can be
    /// found here
    children: Vec<StateTree>,

    /// pointer to the currently selected sub state.
    cursor: usize,
}

impl StateTree {
    fn get_state(&self, cursor: &[usize]) -> &State {
        if cursor.is_empty() {
            return &self.state;
        }

        self.children[cursor[0]].get_state(&cursor[1..])
    }
}

type AnyBox = Box<dyn Any + Send>;
#[derive(Default)]
struct State {
    /// each state holds multible state registers that can be retrieved one after another
    registers: RwLock<Vec<AnyBox>>,
}

impl State {
    fn use_state<T>(&self, value: T, index: usize) -> T
    where
        T: 'static + Clone + Send,
    {
        let head = {
            let v = self.registers.read().expect("to read length of state");
            v.len()
        };
        assert!(index <= head);

        // if this State got called the first time from a Hook, we want to insert the value into a
        // new state register
        if head == index {
            let mut state = self.registers.write().expect("to write value to state");
            state.push(Box::new(value) as Box<dyn Any + Send>);
        }

        // retrieve value from state
        let state = self.registers.read().expect("to read value from state");
        state[index]
            .downcast_ref::<T>()
            .expect(&format!(
                "state hook #{} to be of type {}",
                index,
                std::any::type_name::<T>()
            ))
            .clone()
    }
}

#[derive(Default)]
struct Hooks {
    // TODO Reference counted [usize] might be a better fit in order to avoid cloning in set_value
    // closure
    /// points to State in global StateTree
    cursor: Vec<usize>,
    /// points to the next state register (of state referenced by cursor) to be retrieved
    counter: usize,
}

impl Hooks {
    fn use_state<T>(&mut self, value: T) -> (T, impl Fn(T))
    where
        T: 'static + Clone + Send,
    {
        // index is the currently active state register
        let index = self.counter;

        // increment the counter so the next call to use_state will point to the following state register
        self.counter += 1;

        // retrieve state pointed to by hook
        let tree: MutexGuard<'_, StateTree> = STATE_TREE.lock().expect("to read global StateTree");
        let state = tree.get_state(&self.cursor);

        // retrieve value from state, replacing value this function was called with.
        let value = state.use_state(value, index);

        let cursor = self.cursor.clone();

        let set_value = move |value: T| {
            let tree = STATE_TREE.lock().expect("to read global StateTree");
            let state = tree.get_state(&cursor);

            let mut registers = state
                .registers
                .write()
                .expect("to write updated value to state");

            registers[index] = Box::new(value);
        };

        (value, set_value)
    }
}
