use clap::Parser;
use halo2_base::gates::{GateChip, GateInstructions, RangeChip, RangeInstructions};
use halo2_base::utils::{ScalarField};
use halo2_base::AssignedValue;
use halo2_base::{
    Context,
    QuantumCell::{Constant},
};
use halo2_scaffold::scaffold::cmd::Cli;
use halo2_scaffold::scaffold::run;
use serde::{Deserialize, Serialize};
use std::env::var;
use std::vec;

const MAX_PATTERN_LEN: usize = 3;
const MAX_INPUT_LEN: usize = 3;
const MAX_THREADS: usize = 16;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CircuitInput {
    pub pattern: String,
    pub input_string: String,
    pub pattern_len: u64,
    pub input_len: u64,
}

// #[derive(Debug, Copy, Clone)]
// enum Op {
//     Char = 1,
//     Split = 2,
//     Jmp = 3,
//     Match = 4,
// }

// #[derive(Debug, Copy, Clone)]
// struct Instruction<F: ScalarField> {
//     op: Op,
//     operand_1: AssignedValue<F>,
//     operand_2: AssignedValue<F>,
//     instr_num: AssignedValue<F>,
// }

// #[derive(Debug, Copy, Clone)]
// struct Thread<F: ScalarField> {
//     pc: AssignedValue<F>,  // Program Counter
//     sp: AssignedValue<F>,  // String Pointer
// }

fn get_field<F: ScalarField>(ctx: &mut Context<F>,
    gate: &GateChip<F>,
    code_sequence: &Vec<Vec<AssignedValue<F>>>,
    pc: &AssignedValue<F>,
    i: usize) -> AssignedValue<F> {
    let one_hot_vector = code_sequence.iter().map(|instr| {
        gate.is_equal(ctx, instr[3], *pc)
    }).collect::<Vec<_>>();
    let opcodes = code_sequence.iter().map(|instr| instr[i]).collect::<Vec<_>>();
    gate.select_by_indicator(ctx, opcodes, one_hot_vector)
}

fn get_opcode<F: ScalarField>(ctx: &mut Context<F>,
    gate: &GateChip<F>,
    code_sequence: &Vec<Vec<AssignedValue<F>>>,
    pc: &AssignedValue<F>) -> AssignedValue<F> {
    get_field(ctx, gate, code_sequence, pc, 0)
}

fn get_operand1<F: ScalarField>(ctx: &mut Context<F>,
    gate: &GateChip<F>,
    code_sequence: &Vec<Vec<AssignedValue<F>>>,
    pc: &AssignedValue<F>) -> AssignedValue<F> {
    get_field(ctx, gate, code_sequence, pc, 1)
}

fn get_operand2<F: ScalarField>(ctx: &mut Context<F>,
    gate: &GateChip<F>,
    code_sequence: &Vec<Vec<AssignedValue<F>>>,
    pc: &AssignedValue<F>) -> AssignedValue<F> {
    get_field(ctx, gate, code_sequence, pc, 2)
}

fn add_thread<F: ScalarField>(ctx: &mut Context<F>,
    gate: &GateChip<F>,
    thread_list: &Vec<Vec<AssignedValue<F>>>,
    pc: &AssignedValue<F>,
    sp: &AssignedValue<F>,
    avail: &AssignedValue<F>) -> Vec<Vec<AssignedValue<F>>> {
    let ret = (0..MAX_THREADS).map(|i| {
        let sel = gate.is_equal(ctx, *avail, Constant(F::from(i as u64)));
        let invalid_pc = gate.is_zero(ctx, *pc);
        let valid_pc = gate.not(ctx, invalid_pc);
        let sel = gate.and(ctx, sel, valid_pc);
        let pc = gate.select(ctx, *pc, thread_list[i][0], sel);
        let sp = gate.select(ctx, *sp, thread_list[i][1], sel);
        vec![pc, sp]
    }).collect::<Vec<_>>();
    ret
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


    let mut code_sequence = Vec::new();
    let mut instr_num = ctx.load_constant(F::from(1));
    for i in 0..(MAX_PATTERN_LEN - 1) {
        let c = pattern[i];
        let valid = range.is_less_than(ctx, Constant(F::from(i as u64)), pattern_len, 10);

        // check if the character is asterisk
        let is_asterisk = gate.is_equal(ctx, pattern[i], Constant(F::from('*' as u64)));
        let invalid = gate.not(ctx, valid);
        let is_asterisk = gate.or(ctx, invalid, is_asterisk);

        // check if the character is followed by an asterisk
        let after = if i < MAX_PATTERN_LEN - 1 { pattern[i+1] } else { ctx.load_zero() };
        let followed_by_asterisk = gate.is_equal(ctx, after, Constant(F::from('*' as u64)));

        let temp_selector = gate.or(ctx, is_asterisk, followed_by_asterisk);
        let temp_selector = gate.not(ctx, temp_selector);

        // First instruction
        let instr_1_candidate1 = vec![ctx.load_constant(F::from(2)),
                                                            gate.add(ctx, Constant(F::one()), instr_num),
                                                            gate.add(ctx, Constant(F::from(3)), instr_num),
                                                            instr_num];
        let instr_1_candidate2 = vec![ctx.load_constant(F::one()), c, ctx.load_zero(), instr_num];
        let instr_1 = (0..4).map(|i| {
            let temp = gate.mul(ctx, instr_1_candidate1[i], followed_by_asterisk);
            let temp2 = gate.mul(ctx, instr_1_candidate2[i], temp_selector);
            gate.add(ctx, temp, temp2)
        }).collect::<Vec<_>>();

        let inc = gate.sub(ctx, Constant(F::from(1)), is_asterisk);
        instr_num = gate.add(ctx, instr_num, inc);
        code_sequence.push(instr_1);

        // Second instruction
        let instr_2_candidate = vec![ctx.load_constant(F::one()), pattern[i], ctx.load_zero(), instr_num];
        let instr_2 = instr_2_candidate.into_iter().map(|x|
            gate.mul(ctx, followed_by_asterisk, x)).collect::<Vec<_>>();

        instr_num = gate.add(ctx, instr_num, followed_by_asterisk);
        code_sequence.push(instr_2);

        // Third instruction
        let instr_3_candidate = vec![ctx.load_constant(F::from(3)),
                                                            gate.sub(ctx, instr_num, Constant(F::from(2))),
                                                            ctx.load_zero(),
                                                            instr_num];
        let instr_3 = instr_3_candidate.into_iter().map(|x|
            gate.mul(ctx, followed_by_asterisk, x)).collect::<Vec<_>>();
        instr_num = gate.add(ctx, instr_num, followed_by_asterisk);
        code_sequence.push(instr_3);
    }
    code_sequence.push(vec![ctx.load_constant(F::from(4)), ctx.load_zero(), ctx.load_zero(), instr_num]);

    let mut thread_list = (0..MAX_THREADS).map(|i| {
        if i == 0 { vec![ctx.load_constant(F::one()), ctx.load_zero()] }
        else { vec![ctx.load_zero(), ctx.load_zero()] }
    }).collect::<Vec<_>>();

    let mut avail = ctx.load_constant(F::one());
    let mut out = ctx.load_zero();
    for i in 0..MAX_THREADS {
        let mut pc = thread_list[i][0];
        let mut sp = thread_list[i][1];
        for _ in 0..MAX_INPUT_LEN * 4 {
            let opcode = get_opcode(ctx, &gate, &code_sequence, &pc);
            let operand1 = get_operand1(ctx, &gate, &code_sequence, &pc);
            let operand2 = get_operand2(ctx, &gate, &code_sequence, &pc);

            let char_sel = gate.is_equal(ctx, opcode, Constant(F::from(1)));
            let split_sel = gate.is_equal(ctx, opcode, Constant(F::from(2)));
            let jmp_sel = gate.is_equal(ctx, opcode, Constant(F::from(3)));
            let match_sel = gate.is_equal(ctx, opcode, Constant(F::from(4)));

            // Matched string
            let reached_end = gate.is_equal(ctx, sp, input_len);
            let success = gate.and(ctx, match_sel, reached_end);
            out = gate.or(ctx, out, success);

            // Change program counter
            let change_pc = gate.or(ctx, split_sel, jmp_sel);
            let pc_inc = gate.add(ctx, pc, Constant(F::one()));
            pc = gate.select(ctx, operand1, pc_inc, change_pc);

            // Check if char matches if current instruction is char c
            let cur_char = gate.select_from_idx(ctx, input_string.clone(), sp);
            let char_match = gate.is_equal(ctx, cur_char, operand1);
            let char_is_dot = gate.is_equal(ctx, operand1, Constant(F::from('.' as u64)));
            let match_success = gate.or(ctx, char_match, char_is_dot);
            let not_char_instr = gate.not(ctx, char_sel);
            let match_success = gate.or(ctx, match_success, not_char_instr);
            pc = gate.mul(ctx, pc, match_success);

            // Advance string pointer if current instruction is char c
            sp = gate.add(ctx, sp, char_sel);

            // Add new thread if current instruction is split
            let new_thread_pc = gate.select(ctx, operand2, Constant(F::zero()), split_sel);
            thread_list = add_thread(ctx, &gate, &thread_list, &new_thread_pc, &sp, &avail);
            avail = gate.add(ctx, avail, split_sel);
        }
    }
    make_public.push(out);

    println!("val_assigned: {:?}", out.value());
}

fn main() {
    env_logger::init();

    let args = Cli::parse();

    // run different zk commands based on the command line arguments
    run(regex_parser, args);
}