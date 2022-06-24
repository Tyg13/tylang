use crate::types::*;

pub fn to_string(module: &Module) -> String {
    let mut out = Vec::new();
    module_(&mut out, module).unwrap();
    String::from_utf8(out).unwrap()
}

fn join<T>(
    out: &mut dyn std::io::Write,
    sep: &str,
    ts: impl Iterator<Item = T>,
    mut each: impl FnMut(&mut dyn std::io::Write, T) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let mut first = true;
    for t in ts {
        if !first {
            write!(out, "{sep}")?;
        }
        first = false;
        each(out, t)?;
    }
    Ok(())
}

fn module_(out: &mut dyn std::io::Write, module: &Module) -> std::io::Result<()> {
    writeln!(out, "Types = {{")?;
    for (idx, ty) in module.types.iter().enumerate() {
        write!(out, "  {idx}: ")?;
        type_(out, ty, &module.types)?;
        write!(out, "\n")?;
    }
    writeln!(out, "}}")?;

    join(out, "\n", module.functions.iter(), |out, function| {
        function_(out, function, &module.types)
    })?;
    Ok(())
}

fn type_(out: &mut dyn std::io::Write, ty: &Type, types: &[Type]) -> std::io::Result<()> {
    match ty {
        Type::Void => write!(out, "void")?,
        Type::Basic { name } => write!(out, "{name}")?,
        Type::Integer { size } => write!(out, "i{size}")?,
        Type::Pointer { target } => {
            write!(out, "*")?;
            type_(out, &types[target.0], types)?;
        }
        Type::Struct { name, members } => {
            write!(out, "{name} {{ ")?;
            join(out, ", ", members.iter(), |out, member| {
                write!(out, "{}: ", member.name)?;
                type_(out, &types[member.typ_.0], types)?;
                Ok(())
            })?;
            write!(out, " }}")?;
        }
    }
    Ok(())
}

fn function_(
    out: &mut dyn std::io::Write,
    function: &Function,
    types: &[Type],
) -> std::io::Result<()> {
    write!(out, "fn {}(", function.name)?;
    let mut had_params = false;
    join(out, ", ", function.parameters.iter(), |out, param| {
        had_params = true;
        param_(out, param, types)
    })?;
    if function.is_var_args {
        if had_params {
            write!(out, ", ")?;
        }
        write!(out, "...")?;
    }
    write!(out, ") -> ")?;
    type_(out, &types[function.return_type.0], types)?;
    if function.instructions.len() > 0 {
        instructions_(out, &function, types)?;
    } else {
        write!(out, ";")?;
    }
    Ok(())
}

fn param_(out: &mut dyn std::io::Write, param: &Parameter, types: &[Type]) -> std::io::Result<()> {
    write!(out, "{}: ", param.name)?;
    type_(out, &types[param.type_.0], types)?;
    Ok(())
}

fn instructions_(
    out: &mut dyn std::io::Write,
    function: &Function,
    types: &[Type],
) -> std::io::Result<()> {
    write!(out, " {{")?;
    let mut first = true;
    for (idx, instruction) in function.instructions.iter().enumerate() {
        if first {
            write!(out, "\n")?;
        }
        if !first && function.blocks.find_vertex(&idx).is_some() {
            writeln!(out)?;
        }
        first = false;
        write!(out, "  {idx}: ")?;
        instruction_(
            out,
            instruction,
            &function.parameters,
            &function.blocks,
            types,
        )?;
        write!(out, "\n")?;
    }
    write!(out, "}}")?;
    Ok(())
}

fn instruction_(
    out: &mut dyn std::io::Write,
    instruction: &Instruction,
    parameters: &[Parameter],
    blocks: &BlockGraph,
    types: &[Type],
) -> std::io::Result<()> {
    match instruction {
        Instruction::Nop => {
            write!(out, "nop")?;
        }
        Instruction::Declaration {
            name,
            type_: ty,
            value,
            promoted,
        } => {
            if *promoted {
                write!(out, "(P)")?;
            }
            if name.is_empty() {
                write!(out, "unnamed ")?;
            } else {
                write!(out, "%{name}: ")?;
            }
            type_(out, &types[ty.0], types)?;
            if let Some(value) = value {
                write!(out, " = ")?;
                value_or_operation_(out, &value, parameters, types)?;
            }
        }
        Instruction::Call { function, operands } => {
            write!(out, "call {function}(")?;
            join(out, ", ", operands.iter(), |out, op| {
                value_(out, op, parameters, types)?;
                Ok(())
            })?;
            write!(out, ")")?;
        }
        Instruction::Jump { target } => {
            write!(out, "jmp @{}", target.data(&blocks))?;
        }
        Instruction::Branch {
            condition,
            left,
            right,
        } => {
            write!(out, "branch ")?;
            value_(out, &condition, parameters, types)?;
            write!(out, " @{}, @{}", left.data(&blocks), right.data(&blocks))?;
        }
        Instruction::Return { value } => {
            write!(out, "return ")?;
            value_(out, &value, parameters, types)?;
        }
        Instruction::Choice {
            left_value,
            left,
            right_value,
            right,
        } => {
            write!(out, "choice [@{}, ", left.data(&blocks))?;
            value_(out, &left_value, parameters, types)?;
            write!(out, "] [@{}, ", right.data(&blocks))?;
            value_(out, &right_value, parameters, types)?;
            write!(out, "]")?;
        }
        Instruction::Truncate { to_type, value } => {
            write!(out, "trunc ")?;
            type_(out, &types[to_type.0], types)?;
            write!(out, " to ")?;
            value_(out, &value, parameters, types)?;
        }
        Instruction::Extend { to_type, value } => {
            write!(out, "extend ")?;
            value_(out, &value, parameters, types)?;
            write!(out, " to ")?;
            type_(out, &types[to_type.0], types)?;
        }
    }
    Ok(())
}

fn value_or_operation_(
    out: &mut dyn std::io::Write,
    value: &ValueOrOperation,
    parameters: &[Parameter],
    types: &[Type],
) -> std::io::Result<()> {
    match value {
        ValueOrOperation::Value(val) => value_(out, &val, parameters, types),
        ValueOrOperation::Operation(op) => op_(out, &op, parameters, types),
    }
}

fn value_(
    out: &mut dyn std::io::Write,
    value: &Value,
    parameters: &[Parameter],
    types: &[Type],
) -> std::io::Result<()> {
    match value {
        Value::Void => write!(out, "void")?,
        Value::Literal(lit) => match lit {
            Literal::Number(n) => write!(out, "${n}")?,
            Literal::Str(s) => write!(out, "\"{s}\"")?,
        },
        Value::VariableRef(idx) => write!(out, "%{idx}")?,
        Value::ParamRef(idx) => {
            let param = &parameters[*idx];
            write!(out, "{}", param.name)?;
        }
    }
    Ok(())
}

fn op_(
    out: &mut dyn std::io::Write,
    op: &Operation,
    parameters: &[Parameter],
    types: &[Type],
) -> std::io::Result<()> {
    fn write_op(
        out: &mut dyn std::io::Write,
        idx: usize,
        op: &Operation,
        parameters: &[Parameter],
        types: &[Type],
    ) -> std::io::Result<()> {
        value_(out, &op.operands[idx], parameters, types)
    }
    let (op_str, num_ops) = match op.kind {
        OperationKind::Add => ("add", 2),
        OperationKind::Subtract => ("sub", 2),
        OperationKind::Multiply => ("mul", 2),
        OperationKind::Divide => ("div", 2),
        OperationKind::LessThan => ("lt", 2),
        OperationKind::LessThanEquals => ("lte", 2),
        OperationKind::GreaterThan => ("gt", 2),
        OperationKind::GreaterThanEquals => ("gte", 2),
        OperationKind::Equals => ("eq", 2),
        OperationKind::Index => ("index", 2),
        OperationKind::Assignment => ("assign", 2),
    };
    write!(out, "{op_str} ")?;
    join(out, ", ", 0..num_ops, |out, idx| {
        write_op(out, idx, op, parameters, types)
    })?;
    Ok(())
}
