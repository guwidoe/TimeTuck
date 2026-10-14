# Owned solution persistence

`SolutionFileIo<C>` reads and writes owned models through an executable
`SolutionCodec<S>`. Reads stop at the configured byte limit. Encoding and graph
validation finish before the destination is touched; a sibling temporary file
then publishes through rename. Replacement follows the host filesystem rules,
with no directory fsync or crash-durability guarantee. A caller codec controls
its own allocation and side effects.

The native versioned JSON and XML codecs carry a `BasicSolutionDocument<S>`:
ordered value and entity codes, optional assignments and a typed score envelope.
Decoding resolves references to canonical `Arc<PersistedValue>` allocations.
Duplicate identities, missing references, incompatible score types and malformed
members reject. Encoding rejects noncanonical references; XML encoding rejects
characters outside XML 1.0. These are explicit native formats, with no generic
Jackson/JAXB mapper or Java wire compatibility claim.

A document opens a real `FiniteProblem<String>`. Committing an assignment
validates the entire assignment and resolves all references before changing
any entity or stored score. Supplied scores remain caller data; persistence does
not independently compute them. Original integration tests execute owned manager
jobs through prepared Dense and Stream directors, independently check returned
scores, then write and reread solved models with shared value identities.

The two complete Community solution-file TEST adaptations preserve separate
solution ownership, exact ordered codes, negative score values and shared value
identity. Helper provenance and notices are recorded; original regression controls
receive no upstream-method credit.
