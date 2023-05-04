use clap::Parser;
use halo2_base::gates::{GateChip, GateInstructions};
use halo2_base::utils::ScalarField;
use halo2_base::AssignedValue;
use halo2_base::{
    Context,
    QuantumCell::{Constant, Existing},
};
use halo2_scaffold::scaffold::cmd::Cli;
use halo2_scaffold::scaffold::run;
use serde::{Deserialize, Serialize};

use std::vec;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CircuitInput {
    pub pattern: String,
    pub input_string: String,
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

    let gate = GateChip::<F>::default();

    let mut transition_table: Vec<Vec<AssignedValue<F>>> = Vec::new();

    for c in pattern {
        let is_equal = gate.is_equal(ctx, c, Constant(F::from('*' as u64)));
        let inc = gate.sub(ctx, Constant(F::from(1)), is_equal);
        if let Some(last_value) = transition_table.last() {
            let last_state = *last_value.last().unwrap();
            let next_state = gate.add(ctx, last_state, inc);
            transition_table.push(vec![c, last_state, next_state]);
        } else {
            let initial_state = ctx.load_constant(F::from(1));
            let next_state = gate.add(ctx, initial_state, inc);
            transition_table.push(vec![c, initial_state, next_state]);
        }
    }

    // transition table:
    // char | cur_state | next_state
    // a    | 1         | 2
    // b    | 2         | 3
    // *    | 3         | 3
    // c    | 3         | 4

    let accept = *transition_table.last().unwrap().last().unwrap();

    let next_states_vec = transition_table.iter().map(|row| Existing(row[2])).collect::<Vec<_>>();
    let mut possible_states = vec![ctx.load_constant(F::from(1))];
    for c in input_string {
        let tokens = vec![Constant(F::from('*' as u64)), Constant(F::from('.' as u64)), Existing(c)];
        let mut res = vec![];
        for x in &possible_states {
            for token in &tokens {
                // Pick the row in the transition table that has the same token and cur_state
                let one_hot_vector = transition_table.iter().map(|row| {
                    let diff1 = gate.sub(ctx, row[0], *token);
                    let diff2 = gate.sub(ctx, row[1], *x);
                    let diff1_iszero = gate.is_zero(ctx, diff1);
                    let diff2_iszero = gate.is_zero(ctx, diff2);
                    gate.mul(ctx, diff1_iszero, diff2_iszero)
                }).collect::<Vec<_>>();
                // Get the next state in that row and push it into the possible_states in the next iteration
                res.push(gate.inner_product(ctx, one_hot_vector.clone(), next_states_vec.clone()));
            }
        }
        std::mem::swap(&mut possible_states, &mut res);
    }

    // Check if the final possible states contain the accept state
    let out_vec = possible_states.into_iter().map(|x| {
        let diff = gate.sub(ctx, x, accept);
        gate.is_zero(ctx, diff)
    }).collect::<Vec<_>>();
    let out = gate.sum(ctx, out_vec);
    make_public.push(out);

    println!("val_assigned: {:?}", out.value());
}

fn main() {
    env_logger::init();

    let args = Cli::parse();

    // run different zk commands based on the command line arguments
    run(regex_parser, args);
}