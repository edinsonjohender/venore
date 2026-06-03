//! Research Engine prompts — system prompts for Manager and Worker agents

/// Manager prompt for decomposing a research seed into hexagons + worker assignments
pub fn decompose_prompt(
    feature_name: &str,
    feature_description: &str,
    objective: &str,
    intensity: &str,
    max_hexagons: i32,
    max_workers: i32,
) -> String {
    format!(
        r#"You are a Research Manager agent. Your job is to decompose a research topic into specific investigation points (hexagons) and assign them to parallel worker agents.

## Research Topic
**Name:** {feature_name}
**Description:** {feature_description}
**Objective:** {objective}
**Intensity:** {intensity}

## Task
Decompose this topic into {max_hexagons} specific research points, then group them into {max_workers} worker assignments. Each worker will investigate its assigned points in parallel.

## Rules
- Each hexagon should be a specific, focused question or investigation point
- Group related hexagons together for the same worker
- Spread the workload roughly evenly across workers
- Prioritize hexagons by importance (most critical research questions first)
- The description should guide the worker on what to look for

## Response Format
Respond with ONLY valid JSON (no markdown, no explanation):
{{
  "hexagons": [
    {{ "title": "...", "description": "...", "priority": "high|medium|low" }}
  ],
  "assignments": [
    {{ "hexagon_indices": [0, 1], "instructions": "Focus on..." }}
  ]
}}"#
    )
}

/// Manager prompt for evaluating worker results and deciding next steps
pub fn evaluate_prompt(
    feature_name: &str,
    objective: &str,
    hexagons_summary: &str,
    evidence_count: usize,
    evaluation_round: i32,
    max_rounds: i32,
    user_instructions: &[String],
) -> String {
    let instructions_section = if user_instructions.is_empty() {
        String::new()
    } else {
        format!(
            "\n## User Instructions (from chat control channel)\n{}\n",
            user_instructions
                .iter()
                .map(|i| format!("- {i}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };

    format!(
        r#"You are a Research Manager agent. Review the current state of the investigation and decide what to do next.

## Research Topic
**Name:** {feature_name}
**Objective:** {objective}
**Evaluation Round:** {evaluation_round}/{max_rounds}
**Total Evidence Collected:** {evidence_count}
{instructions_section}
## Current Hexagon Status
{hexagons_summary}

## Task
Evaluate the research progress and decide:
1. **"continue"** — If important gaps remain, create new hexagons and spawn more workers
2. **"next_phase"** — If the current phase (discover) is complete, advance hexagons to the next phase (define/validate) and spawn workers for deeper investigation
3. **"conclude"** — If enough evidence has been gathered to answer the research objective

## Rules
- Consider the objective: has enough been investigated to achieve it?
- Look for gaps: are there obvious questions not yet covered?
- Check confidence levels: are they high enough?
- Dead ends are fine — they're valuable information
- Round {evaluation_round}/{max_rounds}: if this is the last round, prefer "conclude"

## Response Format
Respond with ONLY valid JSON:
{{
  "decision": "continue|next_phase|conclude",
  "reasoning": "Brief explanation of your decision",
  "gaps": ["Gap description 1", "..."],
  "new_hexagons": [
    {{ "title": "...", "description": "...", "priority": "high|medium|low" }}
  ],
  "assignments": [
    {{ "hexagon_indices": [0, 1], "instructions": "Focus on..." }}
  ],
  "phase_transition": "define|validate|conclude"
}}"#
    )
}

/// Build a summary of hexagons for the manager evaluation prompt
pub fn build_hexagons_summary(
    hexagons: &[crate::knowledge::KnowledgeHexagon],
    evidence_counts: &std::collections::HashMap<String, usize>,
) -> String {
    if hexagons.is_empty() {
        return "No hexagons created yet.".to_string();
    }

    let mut lines = Vec::new();
    for hex in hexagons {
        let ev_count = evidence_counts.get(&hex.id).copied().unwrap_or(0);
        let dead = if hex.is_dead_end { " [DEAD END]" } else { "" };
        lines.push(format!(
            "- **{}** ({}): {}% | confidence={} | risk={} | evidence={}{}\n  Notes: {}",
            hex.title,
            hex.phase,
            hex.percentage,
            hex.confidence,
            hex.risk,
            ev_count,
            dead,
            if hex.notes_user.is_empty() {
                "(none)"
            } else {
                &hex.notes_user
            }
        ));
    }
    lines.join("\n")
}
