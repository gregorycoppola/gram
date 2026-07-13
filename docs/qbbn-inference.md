# QBBN inference semantics and verification

This document separates three questions:

1. What probability distribution does a QBBN define?
2. How does belief propagation approximate or recover its marginals?
3. Which tests establish correctness?

## Exact model semantics

Exact inference is the semantic reference implementation for small graphs.

A complete assignment receives weight from:

- a Bernoulli prior for each source proposition;
- deterministic AND factors for rule-premise groups;
- log-linear conditional tables for proposition OR factors;
- unary likelihood factors for observations.

The complete finite distribution is globally normalized.

For query variable \(X\) and evidence \(E\):

\[
P(X=1\mid E)
=
\frac{
    \sum_{\mathbf{x}:X=1}
    \prod_f \psi_f(\mathbf{x}_f)
    \prod_i L_i(x_i)
}{
    \sum_{\mathbf{x}}
    \prod_f \psi_f(\mathbf{x}_f)
    \prod_i L_i(x_i)
}.
\]

Exact inference is exponential and is intended for small examples, tests, and
debugging.

## AND factors

A group variable is deterministically true exactly when all of its premise
literals are true.

Negated premises test the false state of their underlying proposition.

## OR factors

An OR factor combines active positive and negative rule groups.

For accumulated positive and negative scores:

\[
P(X=1\mid \text{groups})
=
\frac{\exp(s_+)}
{\exp(s_+) + \exp(s_-)}.
\]

With no active groups, both scores are zero and the current baseline is 0.5.

## Evidence

An evidence probability \(q\) is treated as a unary likelihood:

\[
L(X=1)=q,\qquad L(X=0)=1-q.
\]

Values 0 and 1 represent hard evidence.

With a uniform root prior, soft evidence \(q\) produces posterior \(q\).

## Belief propagation

BP uses edge-specific sum-product messages.

A variable-to-factor message excludes the message received from its target
factor. This cavity-message rule prevents immediate double counting.

A factor-to-variable message enumerates the other variables in the factor,
multiplies their incoming messages by the factor potential, and marginalizes
them out.

## Topology contract

Topology is measured on the undirected bipartite graph whose nodes are:

- QBBN variables;
- QBBN factors.

Every variable participating in a factor creates one undirected incidence
edge.

### Tree or forest

When the factor graph is acyclic, sum-product BP is exact.

Tests on these graphs must require:

\[
|P_{\mathrm{BP}}(X)-P_{\mathrm{exact}}(X)| < \epsilon.
\]

A disconnected forest is valid because each connected component can be
inferred independently.

### Loopy graph

When the factor graph contains a cycle, the algorithm is loopy BP.

Loopy BP may:

- converge to an inexact fixed point;
- oscillate;
- depend on damping;
- fail to converge within the iteration limit.

Loopy fixtures remain useful diagnostics, but they must not assert that BP
equals the exact marginal.

They should report:

- whether BP converged;
- iteration count;
- exact marginal;
- BP marginal;
- signed and absolute error.

## Test categories

### Exact semantic tests

Compare exact enumeration with hand calculations.

These establish the intended probabilistic semantics independently of BP.

### Acyclic BP tests

First assert that the factor graph is acyclic.

Then require BP to equal exact inference.

Coverage should include:

- forward inference;
- downstream evidence and backward inference;
- multiple independent children;
- conjunction;
- negated premises;
- multi-step chains;
- soft evidence.

### Loopy diagnostics

First assert that the factor graph is cyclic.

Run exact inference when the graph is small enough, then record BP error
without requiring equality.

### Metamorphic tests

Useful model-independent properties include:

- assignment probabilities normalize to one;
- every marginal lies in \([0,1]\);
- \(P(A)+P(\neg A)=1\);
- reordering factors does not change exact results;
- adding a disconnected component does not change an existing marginal;
- damping does not change the final result on an acyclic graph;
- increasing a positive rule weight should not reduce its supported
  conclusion in a simple monotonic tree.