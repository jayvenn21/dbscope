# Risk model (deterministic scoring)

DBScope uses two risk scores: **table risk** (schema-only) and **impact (blast radius) risk**. Both are deterministic, documented, and in the range 0–1.

---

## 1. Table risk (per-table schema risk)

**Purpose:** How structurally critical is this table? (FK depth, cycles, centrality.)

**Formula:**

```
risk = depth_contrib + cycle_contrib + centrality_contrib   (capped at 1.0)
```

| Component        | Max  | How it’s computed |
|-----------------|------|-------------------|
| **depth_contrib**   | 0.40 | `(fk_depth_out + fk_depth_in) / 20`, capped at 0.4 |
| **cycle_contrib**   | 0.30 | 0.3 if table is in a circular FK dependency, else 0 |
| **centrality_contrib** | 0.30 | `(centrality_in + centrality_out) / 30`, capped at 0.3 |

- **FK depth (out):** Max path length following *outgoing* FK edges (tables this table references).
- **FK depth (in):** Max path length following *incoming* FK edges (tables that reference this one).
- **Centrality in/out:** Number of direct FK neighbors (in-degree and out-degree in the FK graph).

**Orphans:** Tables with no FK in and no FK out get `risk = 0` and a breakdown explaining “orphan”.

**Interpretation:**

| Score range | Label    | Meaning |
|-------------|----------|---------|
| 0.75 – 1.0  | Critical | Very central and/or deep in FK chain and/or in a cycle |
| 0.50 – 0.75 | High    | High centrality or depth |
| 0.25 – 0.50 | Moderate | Some dependency depth or centrality |
| 0 – 0.25    | Low     | Few dependencies, shallow in graph |
| 0           | (Orphan) | No FK in or out |

**Why “0.25” is moderate:** The weighting caps each term (depth 0.4, cycle 0.3, centrality 0.3). A score of 0.25 typically means non-trivial depth or centrality but not at the cap. So 0.25 is “meaningful but not extreme” → Moderate.

---

## 2. Impact (blast radius) risk

**Purpose:** If I change or drop *this* table/column, how large is the impact? (Downstream tables, indexes, observed queries.)

**Formula:**

```
risk_delta = fk_downstream_contrib + index_contrib + queries_contrib   (capped at 1.0)
```

| Component | Weight | How it’s computed |
|-----------|--------|-------------------|
| **FK reach (downstream)** | 0.40 | `min(fk_downstream_count, 20) / 20 * 0.4` |
| **Index coupling**        | 0.30 | `min(index_count, 10) / 10 * 0.3` |
| **Query usage weight**    | 0.30 | `min(queries_affected, 50) / 50 * 0.3` (0 if no query log) |

- **fk_downstream_count:** Number of tables that depend on this table via FK (transitive).
- **index_count:** Number of indexes on this table (or touching the target column).
- **queries_affected:** Number of queries in the provided log that reference the target (table/column).

**Interpretation:**

| Score range | Label    | Meaning |
|-------------|----------|---------|
| 0.75 – 1.0  | Critical | Many downstream tables and/or indexes and/or query usage |
| 0.50 – 0.75 | High    | Large blast radius |
| 0.25 – 0.50 | Moderate | Non-trivial impact (e.g. several tables, some queries) |
| 0 – 0.25    | Low     | Small blast radius |

**Example: 0.25 (Moderate)**  
Typical case: a few downstream tables (e.g. 9), some index coupling, and some queries affected (e.g. 7). The normalization (cap at 20 tables, 10 indexes, 50 queries) keeps the score in a comparable band; 0.25 means “meaningful impact, worth reviewing” without being extreme.

---

## 3. Normalization and caps

- **Table risk:** Depth and centrality are normalized by fixed divisors (20 and 30) and then capped so no single factor dominates.
- **Impact risk:** Downstream count is capped at 20, index count at 10, queries at 50, so scores remain comparable across schemas of different sizes.

All math is deterministic: same schema + same query log → same scores.

---

## 4. Where this appears

- **CLI:** `dbscope impact` prints risk level (e.g. `Moderate (0.25)`), then a breakdown (FK reach, Index coupling, Query usage weight).
- **Reports:** HTML has a “Risk scoring (explainable)” section; JSON includes `risk_breakdown` per table and in impact outputs when available.
- **Reproducibility:** No randomness; only inputs are schema metadata and the optional query log.
