# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

The developer following the TimeTuck Rust rewrite and its progress against the
pinned Timefold Community test suite.

## Product Purpose

Show the current number and percentage of passing adapted upstream tests by
subsystem, and measured performance against the original Java implementation.

## Operating Context

A development dashboard inside this Rust repository. Generated verification
reports are its source of truth. The user explicitly prefers a visual overview;
individual test details remain accessible.

The completion view combines feature coverage, behavior checks, measured Java
performance parity and verification freshness using explicit project weights.
It shows remaining targets and distinguishes last-demonstrated evidence from
current verification; it does not estimate remaining engineering effort or time.

## Capabilities and Constraints

Never equate source-file counts with tests or adapted methods with all JUnit
runtime invocations. Pending, failed, excluded, and unverified methods remain
visible. Benchmark claims apply only to the measured workload. The Community
reference remains pinned and unmodified. No invented measurements.

## Implementation assumptions

Use a local static web page with generated JSON, without a framework or hosting
service. This is an implementation choice for this session, not a user brand or
stack preference. The page refreshes its report while open.
