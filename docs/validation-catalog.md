# Native validation catalog and execution boundary

`ValidationTypeCatalog` registers owned issue descriptions against actual Rust
`TypeId` values. Duplicate codes fail before replacing the prior registration.
The catalog writes real JSON issue descriptors to an explicitly supplied path;
those files describe API data and do not load types or callbacks.

`ValidatedModelRunner` invokes an executable typed validator on the caller's actual
model and configuration. It checks registered codes, severity and result status.
Errors reject execution; warnings are retained alongside a successful output.
Validator failures and execution failures remain distinct. The callback can run
an existing solver or manager; the boundary itself neither creates a solver nor
mutates the model. Caller callback side effects and panics are not rolled back.

The Community deployment TEST adaptations preserve validation issue count,
code, severity, payload id, catalog metadata, descriptor readback and duplicate
code rejection. Explicit native registration replaces deployment scanning/CDI;
a supplied destination replaces Java build-directory conventions. No Quarkus,
class loading, annotation discovery or Java wire/ABI compatibility is claimed.
Original controls run real dense and stream solving with live ranges/configuration,
retain warnings and independently rescore assignments, and prove invalid validation
results cannot reach execution. Originals receive no source-method credit.
