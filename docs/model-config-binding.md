# Native model configuration bindings

`ModelConfigBinding<T>` registers actual typed member getters and setters in
descriptor order. Each `ModelConfigValue` supplies its codec and structural schema;
member options provide external ID/title/description, format, nullable restriction,
role and an optional constraint reference. Schema customization never changes the
typed storage. Weight roles are explicit; a constraint reference defaults to a
weight, with a parameter override available. There is no Rust field discovery or
Java annotation/class scanning.

The descriptor serializes under `modelConfigDescriptor`, supports generated file
writing/readback, and retains ordered parameters. Generated JSON Schema has a
root `$defs` entry, nested named object definitions, composed array item schemas,
nullable alternatives and duration formats. Fixed Rust arrays declare their exact
length. Root properties are optional because omitted inputs preserve constructor
defaults or current values. Custom value codecs own nested required properties
and their validation; a schema format is metadata, not an added validation engine.

`decode` parses unknown properties and every supplied value before invoking the
constructor or any setters. `apply_json` follows the same preflight against an
existing record. Invalid input leaves it unchanged. Missing members remain as
they were; explicit null requires optional storage and allowed nullable metadata.
Duplicate JSON keys follow serde_json's last-value policy. Callback side effects
and setter panics are caller-owned and do not have transactional rollback.

`encode` invokes live getters and checks their returned wire values through the
matching typed codec. Integer range and duration parsing are strict. Decimal
numbers preserve mathematical value, not BigDecimal scale identity. The existing
JSON backend lacks arbitrary precision: raw numeric lexemes that would change
mathematical value during parsing, and Decimal outputs it cannot represent
exactly, are rejected. Cargo features and the solver's exact Decimal arithmetic
are unchanged. Inputs such as `0.125`, `1.25` and `1e-20` bind; a longer decimal
that shortens to a different number is an error.

`constraint_weights` selects named weight members and converts their actual values
to the caller's native score type. All selected IDs, roles and unique scoring
targets validate before getters or score conversion. A reference selects its
constraint name; absent references select the external member ID. Multiple
descriptive references are supported in the descriptor. Selecting competing
weights for the same target is rejected; the caller must select one explicitly.
Parameters are available to the caller's typed solver configuration builder.
The original regressions feed references/renamed step parameters/durations into
real dense and stream local search, and exact decimal weights into live scoring.

The three Community metadata tests preserve all ordered 10/6/3 assertion rows,
after writing and reading a generated descriptor. Native explicit registration
replaces deployment scanning. The runtime controls are original and earn no
additional source-method credit. There is no Quarkus/OpenAPI/JVM deployment,
framework serialization or annotation ABI compatibility claim.
