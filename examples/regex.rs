use clap::Parser;
use halo2_base::gates::{GateChip, GateInstructions};
use halo2_base::utils::ScalarField;
use halo2_base::AssignedValue;
use halo2_base::{
    Context,
    QuantumCell::{Constant, Existing, Witness},
};
use halo2_scaffold::scaffold::cmd::Cli;
use halo2_scaffold::scaffold::run;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CircuitInput {
    pub pattern: String,
    pub input_string: String,
}

use std::collections::HashMap;

// this algorithm takes a public input x, computes x^2 + 72, and outputs the result as public output
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

    let mut dict: HashMap<F, HashMap<F, F>> = HashMap::new();
    let mut state = F::from(0);
    for c in pattern {
        dict.insert(state, {
            let mut entry = HashMap::new();
            let inc = F::from(1) - *gate.is_equal(ctx, c, Constant(F::from('*' as u64))).value();
            state = state + inc;
            entry.insert(*c.value(), state);
            entry
        });
    }

    let accept = ctx.load_witness(state);
    make_public.push(accept);

    // println!("x: {:?}", x.value());
    println!("val_assigned: {:?}", accept.value());
}

fn main() {
    env_logger::init();

    let args = Cli::parse();

    // run different zk commands based on the command line arguments
    run(regex_parser, args);
}