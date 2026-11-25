# Rust style

Write code a person can follow from its names and ownership. Existing violations are debt, not examples.
The [glossary](../architecture/glossary.md) supplies domain vocabulary and allowed abbreviations.
The [lint census](../tasks/0097-rust-style.md#lint-decisions) records the initial decisions and counts.

| Rule | Enforcement |
|---|---|
| Types are nouns describing one concept from the glossary. Functions are verbs describing their effect. | `naming` checks vague words, type synonyms, generic verb prefixes and abbreviations; review checks meaning and verbs. |
| Read a name with its module path. Avoid stutter, similar names and repeated type/field/variant prefixes. | Clippy `module_name_repetitions`, `similar_names`, `struct_field_names`, `enum_variant_names`; review checks API clarity. |
| Each type owns one cohesive job. Compose smaller values; keep callers and callees at a consistent abstraction level. | `size` limits dimensions; review checks cohesion and abstraction. |
| Put traits at real substitution or IO seams. Extend the existing abstraction rather than copying logic. | Reviewed. |
| Use newtypes for units and identifiers that must not be interchangeable. Use enums for states and alternatives. | Reviewed; Clippy `fn_params_excessive_bools`, `struct_excessive_bools` flag boolean-heavy APIs. |
| Prefer iterators to index bookkeeping when they express the operation clearly. | Clippy `needless_range_loop`, `explicit_iter_loop`; review checks readability. |
| Borrow when ownership is unnecessary. Clone only when a distinct owned value is needed. | Clippy `needless_pass_by_value`, `clone_on_copy`, `assigning_clones`; review checks ownership and avoidable allocations. |
| Return typed error enums in new code, never `Result<_, String>`. Convert errors to text at the existing reporting boundary. | Reviewed. |
| Keep each module focused on one responsibility; export the smallest useful API. Follow the architecture module map. | `architecture` checks configured boundaries; review checks cohesion, visibility and placement. |
| Comments explain why, constraints or invariants. Names and code explain what. Document unsafe invariants explicitly. | Clippy `undocumented_unsafe_blocks`, `missing_safety_doc`, `allow_attributes_without_reason`; review checks explanations. |
| Use `Self`, direct pattern matching, early returns and ordinary Rust expressions. | Clippy pedantic plus `use_self`, `option_if_let_else`, `manual_let_else`, `redundant_else`, `unnested_or_patterns`, `semicolon_if_nothing_returned`. |
| Propagate operational errors. Tests may unwrap; production expects state a justified invariant. | Clippy `unwrap_used`, `expect_used` with test exceptions; review checks invariant reasons. |

The `naming` check walks structs, enums, aliases, traits, free functions and inherent methods outside test code.
Trait declarations are checked; trait implementation methods keep the trait's mandated spelling. Macro-generated
names and the meaning of long words require review. Acronyms and words of three letters or fewer must be in the
allowed table; listed longer contractions must be spelled out. It checks declarations rather than local variables.

Workspace lints are warnings during builds. The gate freezes nonzero counts per file and lint; zero is enforced
immediately. A cleanup writes lower counts with `cargo xtask ratchet`. The naming ratchet also keys by declaration
and rule, so a different bad name cannot spend a removed name's allowance. Numeric conversions and a few noisy
annotation lints have explicit, reasoned workspace exceptions. These rules do not require firmware behavior changes.
