# Rust Expression Engine Libraries: 2025 Analysis and Recommendations

## Overview

This document provides a detailed analysis of expression evaluation libraries available in Rust as of 2025. For the DDL project, which previously used the abandoned `eval` crate, this analysis will help determine the best alternative based on features, performance, and suitability for projects that build on DDL.

## Current State: The `eval` Crate

The DDL project previously used the `eval` crate, which has been abandoned since September 2020. The crate description explicitly states: "[ABANDONED] Expression evaluator for Rust."

**Note:** DDL core no longer includes an expression engine. This document is for projects that build expression processing on top of DDL.

### Features of the `eval` Crate

- Basic mathematical operations: `+`, `-`, `*`, `/`, `%`
- Logical operations: `&&`, `||`, `!`, `==`, `!=`, `<`, `>`, `<=`, `>=`
- Array operations with `[]` and `.` accessors
- Built-in functions:
  - `min()` and `max()` for finding minimum/maximum values
  - `len()` for getting length of strings/arrays
  - `is_empty()` for checking if a value is empty
  - `array()` for creating arrays
- Support for variables and custom functions
- MIT license

### Limitations of the `eval` Crate

- No longer maintained
- Limited to JSON-like values (no native support for complex numbers, etc.)
- No advanced features like precompilation or optimization
- No active development or security updates

## Alternative Libraries Analysis

### 1. evalexpr

**Status**: Actively maintained and developed
**License**: AGPL-3.0-only (with commercial licensing available)
**Description**: A powerful arithmetic and boolean expression evaluator with scripting capabilities

#### Features

- Comprehensive operator support:
  - Arithmetic: `+`, `-`, `*`, `/`, `%`, `^` (exponentiation)
  - Comparison: `<`, `>`, `<=`, `>=`, `==`, `!=`
  - Logical: `&&`, `||`, `!`
  - Assignment: `=`, `+=`, `-=`, `*=`, `/=`, `%=`, `^=`, `&&=`, `||=`
- Variable support with type safety (integers, floats, booleans, strings, tuples)
- User-defined functions
- Statement chaining with `;`
- Built-in functions for:
  - Mathematics: `min`, `max`, `floor`, `round`, `ceil`, `abs`, etc.
  - Trigonometry: `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, etc.
  - String operations: `len`, `str::regex_matches`, `str::to_lowercase`, etc.
  - Type checking: `typeof`
- Comments support (C-style: `//` and `/* */`)
- Precompilation for performance optimization
- Serde integration (with feature flag)
- Context system for variable management
- Comprehensive error handling

#### Pros

- Most feature-rich option
- Active development and maintenance
- Strong type safety
- Precompilation capabilities for repeated evaluations
- Extensive documentation with examples
- Good performance for a feature-rich library

#### Cons

- AGPL license may not be suitable for all projects
- More complex than needed for simple mathematical expressions
- Larger dependency footprint

#### Example Usage

```rust
use evalexpr::*;

// Simple evaluation
assert_eq!(eval("1 + 2 + 3"), Ok(Value::from_int(6)));

// With variables
let mut context = HashMapContext::<DefaultNumericTypes>::new();
context.set_value("x".into(), Value::from_float(2.5));
let result = eval_with_context("x * 2 + 1", &context);
assert_eq!(result, Ok(Value::from_float(6.0)));

// With custom functions
let context = context_map! {
    "square" => Function::new(|argument| {
        if let Ok(float) = argument.as_float() {
            Ok(Value::Float(float * float))
        } else {
            Err(EvalexprError::expected_float(argument.clone()))
        }
    })
}.unwrap();
assert_eq!(eval_with_context("square(4)", &context), Ok(Value::from_float(16.0)));
```

#### Performance

Good performance for a feature-rich library. Precompilation significantly improves performance for repeated evaluations.

### 2. fasteval

**Status**: Actively maintained
**License**: MIT
**Description**: A fast and safe expression evaluation library focused on performance

#### Features

- Fast evaluation of algebraic expressions
- Safe execution of untrusted expressions with built-in limits
- Support for interpretation and compiled execution
- Variables and custom functions support
- Standard algebraic operators with short-circuit support:
  - Arithmetic: `+`, `-`, `*`, `/`, `%`, `^` (exponentiation)
  - Comparison: `<`, `>`, `<=`, `>=`, `==`, `!=`
  - Logical: `&&`, `||`, `!`
- Built-in functions for math and string operations
- No external dependencies
- Memory-efficient Slab allocation system
- Compile-to-optimized-form for repeated evaluations
- Optional unsafe variables for maximum performance
- Numeric literal suffixes (K, M, G, T, etc.)

#### Pros

- Excellent performance, especially with compiled expressions
- No dependencies
- Safe evaluation with configurable limits
- Memory-efficient design
- MIT license
- Good for high-frequency evaluation scenarios

#### Cons

- More limited feature set compared to evalexpr
- Less convenient API for complex use cases
- Only works with f64 types

#### Example Usage

```rust
use fasteval::ez_eval;
use std::collections::BTreeMap;

// Simple evaluation
let result = ez_eval("1 + 2 * 3", &mut fasteval::EmptyNamespace).unwrap();
assert_eq!(result, 7.0);

// With variables
let mut map: BTreeMap<String, f64> = BTreeMap::new();
map.insert("x".to_string(), 2.5);
let result = ez_eval("x * 2 + 1", &mut map).unwrap();
assert_eq!(result, 6.0);

// With compilation for repeated evaluation
use fasteval::{Parser, Slab, Evaler, Compiler};
let parser = Parser::new();
let mut slab = Slab::new();
let expr_ref = parser.parse("x * 2 + 1", &mut slab.ps).unwrap().from(&slab.ps);
let compiled = expr_ref.compile(&slab.ps, &mut slab.cs);

map.insert("x".to_string(), 3.0);
let result = fasteval::eval_compiled!(compiled, &slab, &mut map);
assert_eq!(result, 7.0);
```

#### Performance

According to benchmarks included in the documentation:
- 2x faster than evalexpr for safe compiled evaluation
- 3.7x faster than evalexpr for safe interpretation
- Among the fastest libraries for expression evaluation
- Compiled expressions can be 10x+ faster than interpreted ones
- Constant expressions can be 200x+ faster than interpreted ones

### 3. meval

**Status**: Limited maintenance
**License**: Unlicense/MIT dual license
**Description**: A simple math expression parser and evaluator, focused on convenience

#### Features

- Simple math expression parsing and evaluation
- Support for custom variables and functions
- Built-in mathematical functions:
  - Basic math: `sqrt`, `abs`, `exp`, `ln`
  - Trigonometry: `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, etc.
  - Hyperbolic functions: `sinh`, `cosh`, `tanh`, etc.
  - Rounding: `floor`, `ceil`, `round`, `signum`
  - Aggregates: `max`, `min`
- Constants: `pi`, `e`
- Only works with f64 types
- Serde support (with feature flag)

#### Pros

- Simple and easy to use
- Lightweight
- Permissive licensing (Unlicense/MIT)
- Good for basic mathematical expressions

#### Cons

- Only works with f64 types
- Limited feature set compared to evalexpr
- Less active development
- No advanced features like precompilation

#### Example Usage

```rust
// Simple evaluation
let result = meval::eval_str("1 + 2 * 3").unwrap();
assert_eq!(result, 7.0);

// With variables
use meval::{Expr, Context};
let expr: Expr = "x * 2 + 1".parse().unwrap();
let mut ctx = Context::new();
ctx.var("x", 2.5);
let func = expr.bind_with_context(ctx, "x").unwrap();
let result = func(2.5);
assert_eq!(result, 6.0);
```

#### Performance

Moderate performance - faster than simple interpretation but slower than fasteval's compiled mode.

### 4. mexe

**Status**: New/less established
**License**: MIT
**Description**: Minimal arithmetic expression evaluator focused on speed

#### Features

- Simple arithmetic expressions only
- Support for: `+`, `-`, `*`, `/`, `()`, integers, floats
- Minimal feature set
- No variable or function support without custom implementation
- Focus on minimalism and performance

#### Pros

- Very fast for simple arithmetic
- Minimal dependencies
- MIT license
- No allocations when possible

#### Cons

- Very limited feature set
- No variables or functions without custom implementation
- Newer and less proven

#### Example Usage

```rust
let result = mexe::eval("(5 * 8) + 6").unwrap();
assert_eq!(result, 46.0);
```

#### Performance

According to benchmarks in the crate documentation:
- 4-10x faster than meval
- 2x faster than fasteval
- Significantly slower than evalexpr (but evalexpr does much more)

## Feature Comparison

| Feature | evalexpr | fasteval | meval | mexe | eval (current) |
|---------|----------|----------|-------|------|----------------|
| License | AGPL-3.0 | MIT | Unlicense/MIT | MIT | MIT |
| Maintenance | Active | Active | Limited | New | Abandoned |
| Variables | Yes | Yes | Yes | No | Yes |
| Custom Functions | Yes | Yes | Yes | No | Yes |
| Precompilation | Yes | Yes | No | No | No |
| Type Safety | Strong | None (f64 only) | None (f64 only) | None (f64 only) | Weak |
| Control Flow | Yes | Limited | No | No | No |
| Assignments | Yes | No | No | No | Yes |
| Comments | Yes | No | No | No | No |
| Serde Support | Yes (feature) | No | Yes (feature) | No | Yes |
| Performance | Good | Excellent | Fair | Excellent | Unknown |
| Dependencies | Yes | None | Yes | None | Yes |

## DDL-Based Project Requirements Assessment

For projects building expression processing on top of DDL:

1. **Fast evaluation of mathematical expressions**: All libraries meet this
2. **Support for variables**: Required by all alternatives except mexe
3. **Custom function support**: Required by all alternatives except mexe
4. **Error handling with meaningful messages**: All libraries provide this
5. **Zero-copy or minimal allocation for performance**: fasteval and mexe excel here
6. **Serialization/deserialization support**: evalexpr and meval offer this

## Recommendations

### Primary Recommendation: evalexpr

For projects building on DDL, **evalexpr** is the recommended expression engine because:

1. **Feature Compatibility**: It provides the closest match to the current `eval` crate's functionality
2. **Active Development**: Unlike the abandoned `eval` crate, evalexpr is actively maintained
3. **Variable Support**: Full support for variable patterns (similar to DDL topic names)
4. **Custom Functions**: Complete support for user-defined functions matching sqrt, abs, pow, max, min requirements
5. **Precompilation**: Offers performance optimization for repeated evaluations
6. **Error Handling**: Comprehensive error types with meaningful messages
7. **Migration Path**: Easiest transition from the current `eval` crate

### Alternative Recommendation: fasteval

If performance is the primary concern and the project can work within fasteval's constraints:

1. **Performance**: Superior performance, especially with compiled expressions
2. **Safety**: Built-in protections against malicious expressions
3. **Memory Efficiency**: Efficient Slab allocation system
4. **MIT License**: Permissive licensing

However, it would require more significant code changes to implement the full feature set currently provided by the `eval` crate.

### Not Recommended: meval and mexe

- **meval**: Too limited in features for DDL-based expression processing
- **mexe**: Lacks required features like variables and custom functions

## Migration Considerations

### From `eval` to `evalexpr`

1. **API Changes**:
   - Different function names (`eval` vs `evalexpr::eval`)
   - Different context management system
   - Different error types

2. **Expression Compatibility**:
   - Most mathematical expressions should work with minimal changes
   - Variable access patterns are similar
   - Function registration is different but comparable

3. **Performance Improvements**:
   - Precompilation opportunities for frequently used expressions
   - Better memory management with contexts

### From `eval` to `fasteval`

1. **API Changes**:
   - Significantly different API structure
   - Namespace-based variable system
   - Slab-based memory management

2. **Expression Compatibility**:
   - Mathematical expressions will work with minor syntax adjustments
   - More complex expressions might need restructuring

3. **Performance Improvements**:
   - Significant performance gains, especially with compilation
   - Memory-efficient evaluation

## Conclusion

For DDL-based projects, **evalexpr** is the best choice for expression evaluation. It provides the most comprehensive feature set, maintains API similarities, and offers an active development community. The migration should be relatively straightforward, and it provides capabilities for future expansion.

If performance becomes a critical bottleneck, **fasteval** would be the next best option, but it would require more significant implementation changes.

Both **meval** and **mexe** are not suitable for DDL-based projects due to their limited feature sets that don't meet the requirements for variables and custom functions.

**Note:** DDL core does not include expression evaluation. This is intentional - DDL provides a simple log foundation, and expression engines can be built on top as separate, optional components.