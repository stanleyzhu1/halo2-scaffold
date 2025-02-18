use clap::Parser;
use halo2_base::gates::{GateChip, GateInstructions, RangeChip, RangeInstructions};
use halo2_base::utils::{ScalarField};
use halo2_base::AssignedValue;
use halo2_base::QuantumCell;
use halo2_base::{
    Context,
    QuantumCell::{Constant, Existing},
};
use halo2_scaffold::scaffold::cmd::Cli;
use halo2_scaffold::scaffold::run;
use serde::{Deserialize, Serialize};
use std::env::var;
use std::vec;

const MAX_PATTERN_LEN: usize = 20;
const MAX_INPUT_LEN: usize = 20;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CircuitInput {
    pub pattern: String,
    pub input_string: String,
    pub pattern_len: u64,
    pub input_len: u64,
}

// Addes the to_add state to the states bit vector
fn add_state<F: ScalarField>(ctx: &mut Context<F>,
    gate: &GateChip<F>,
    states: &Vec<AssignedValue<F>>,
    to_add: &AssignedValue<F>) -> Vec<AssignedValue<F>> {
    let indicator = gate.idx_to_indicator(ctx, *to_add, MAX_PATTERN_LEN);
    (0..MAX_PATTERN_LEN).map(|i| {
        gate.or(ctx, states[i], indicator[i])
    }).collect::<Vec<_>>()
}


// Look up the transition given the current state is state, and the current
// character is character
fn lookup_transition<F: ScalarField>(ctx: &mut Context<F>,
    gate: &GateChip<F>,
    transition_table: &Vec<Vec<AssignedValue<F>>>,
    state: QuantumCell<F>,
    character: QuantumCell<F>) -> AssignedValue<F> {
    let next_states_vec = transition_table.iter().map(|row| Existing(row[1])).collect::<Vec<_>>();
    let big_value = Constant(F::from(2_u64.pow(10)));
    let key = gate.mul_add(ctx, character, big_value, state);
    let one_hot_vector = transition_table.iter().map(|row| {
        gate.is_equal(ctx, key, row[0])
    }).collect::<Vec<_>>();
    gate.inner_product(ctx, one_hot_vector.clone(), next_states_vec.clone())
}

// Finds the epsilon closure of the states bit vector, i.e. the states that can
// be reached via epsilon edges
fn epsilon_closure<F: ScalarField>(ctx: &mut Context<F>,
    gate: &GateChip<F>,
    transition_table: &Vec<Vec<AssignedValue<F>>>,
    states: &Vec<AssignedValue<F>>) -> Vec<AssignedValue<F>> {
    let mut cur_states = states.clone();

    for _ in 0..(MAX_PATTERN_LEN / 2) {
        let mut next: Vec<AssignedValue<F>> = cur_states.clone();
        for i in 1..MAX_PATTERN_LEN {
            let state_exists = gate.is_equal(ctx, cur_states[i], Constant(F::from(1)));
            let index = lookup_transition(ctx, gate, transition_table, Constant(F::from(i as u64)), Constant(F::from('*' as u64)));
            let to_add = gate.mul(ctx, index, state_exists);
            next = add_state(ctx, &gate, &next, &to_add);
        }
        std::mem::swap(&mut cur_states, &mut next);
    }
    cur_states
}

fn regex_parser<F: ScalarField>(
    ctx: &mut Context<F>,
    input: CircuitInput,
    make_public: &mut Vec<AssignedValue<F>>) {
    let pattern = input.pattern.chars().map(|c| F::from(c as u64)).collect::<Vec<_>>();
    let input_string = input.input_string.chars().map(|c| F::from(c as u64)).collect::<Vec<_>>();

    let pattern = pattern.into_iter().map(|c| ctx.load_witness(c)).collect::<Vec<_>>();
    for c in pattern.clone() {
        make_public.push(c);
    }
    let input_string = input_string.into_iter().map(|c| ctx.load_witness(c)).collect::<Vec<_>>();
    let pattern_len = ctx.load_witness(F::from(input.pattern_len));
    let input_len = ctx.load_witness(F::from(input.input_len));

    let gate = GateChip::<F>::default();
    let lookup_bits =
        var("LOOKUP_BITS").unwrap_or_else(|_| panic!("LOOKUP_BITS not set")).parse().unwrap();
    let range = RangeChip::<F>::default(lookup_bits);

    let mut transition_table: Vec<Vec<AssignedValue<F>>> = Vec::new();

    // transition table:
    // char | cur_state | next_state
    // a    | 1         | 2
    // b    | 2         | 2
    // *    | 2         | 3
    // c    | 3         | 4

    // initial state is 1
    // 0 is used as a failure state (not used in transitions)
    let mut state = ctx.load_constant(F::from(1));
    let mut accept = state;

    let big_value = ctx.load_constant(F::from(2_u64.pow(10)));

    for i in 0..(MAX_PATTERN_LEN - 1) {
        let c = pattern[i];
        let valid = range.is_less_than(ctx, Constant(F::from(i as u64)), pattern_len, 10);
        state = gate.mul(ctx, state, valid);
        // check if the character is followed by an asterisk
        let after = if i < MAX_PATTERN_LEN - 1 { pattern[i+1] } else { ctx.load_zero() };
        let followed_by_asterisk = gate.is_equal(ctx, after, Constant(F::from('*' as u64)));
        // Increment the state if it's not followed by an asterisk
        let inc = gate.sub(ctx, Constant(F::from(1)), followed_by_asterisk);
        let next_state: AssignedValue<F> = gate.add(ctx, state, inc);
        // transition_table.push(vec![c, state, next_state]);
        let key = gate.mul_add(ctx, c, big_value, state);
        transition_table.push(vec![key, next_state]);
        // Only consider the valid part of the string
        accept = gate.select(ctx, next_state, accept, valid);
        state = next_state;
    }

    let initial_state = (0..MAX_PATTERN_LEN).map(|i| {
        if i == 1 { ctx.load_constant(F::from(1)) } else { ctx.load_zero() }
    }).collect::<Vec<_>>();

    let mut possible_states = epsilon_closure(ctx, &gate, &transition_table, &initial_state);

    for i in 0..MAX_INPUT_LEN {
        let mut next_states = [(); MAX_PATTERN_LEN].map(|_| ctx.load_zero()).to_vec();
        let valid = range.is_less_than(ctx, Constant(F::from(i as u64)), input_len, 10);
        let character = input_string[i];
        for j in 0..MAX_PATTERN_LEN {
            let state_exists = possible_states[j];
            let transition1 = lookup_transition(ctx, &gate, &transition_table, Constant(F::from(j as u64)), Existing(character));
            // states that can be reached through a dot edge
            let transition2 = lookup_transition(ctx, &gate, &transition_table, Constant(F::from(j as u64)), Constant(F::from('.' as u64)));
            // only add the state if states_exists = 1
            let to_add1 = gate.mul(ctx, transition1, state_exists);
            let to_add2 = gate.mul(ctx, transition2, state_exists);
            next_states = add_state(ctx, &gate, &next_states, &to_add1);
            next_states = add_state(ctx, &gate, &next_states, &to_add2);
        }
        next_states = epsilon_closure(ctx, &gate, &transition_table, &next_states);
        possible_states = (0..MAX_PATTERN_LEN).into_iter().map(|k| {
            gate.select(ctx, next_states[k], possible_states[k], valid)
        }).collect::<Vec<_>>();
    }

    // Check if the final possible states contain the accept state
    let out = gate.select_from_idx(ctx, possible_states, accept);
    make_public.push(out);

    println!("val_assigned: {:?}", out.value());
}

fn main() {
    env_logger::init();

    let args = Cli::parse();

    // run different zk commands based on the command line arguments
    run(regex_parser, args);
}