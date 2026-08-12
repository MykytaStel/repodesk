# RepoDesk Visual Language

RepoDesk is an AI engineering cockpit, not an analytics dashboard and not a chat wrapper. The interface must help an engineer understand the active Work Item, the current workflow state, what an AI worker can see or change, and the next safe action without making every internal subsystem compete for attention.

This document is a product contract for UI work. New features should follow it unless a deliberate design review changes the contract.

## Product character

The target is **calm technical**:

- more character than a generic enterprise admin panel;
- less decoration than a marketing site;
- dense where engineers inspect data;
- spacious between distinct decisions and semantic groups;
- confident use of one product accent;
- strong hierarchy without persistent visual noise.

RepoDesk should feel closer to a modern developer tool than to a collection of SaaS cards.

## Core rules

### 1. Focus must be earned

Not every feature deserves permanent screen space. The strongest contrast belongs to:

1. the active Work Item;
2. the current workflow phase or blocker;
3. the primary next action;
4. critical safety or integrity state.

Navigation, historical metrics, secondary tools, and advanced orchestration should recede until requested.

Do not add a card merely because a metric exists.

### 2. Prefer intent-centric navigation

Primary surfaces describe what the engineer is trying to do:

- Work
- Code
- Changes
- Runs
- Projects

Lower-level concepts such as models, routing, tokens, memory, agents, audit, and provider health are contextual tools or advanced surfaces. They must not compete with the primary workflow by default.

### 3. Use a small surface hierarchy

Prefer three functional levels:

- **Canvas** — quiet application background.
- **Work/content plane** — solid, readable surface where engineering decisions happen.
- **Chrome/transient layer** — navigation, drawers, popovers, command palette.

Avoid recursive cards inside cards. Use separators, grouping, typography, and whitespace before introducing another elevated container.

### 4. Material belongs to chrome, not content

Blur/translucency may be used sparingly on navigation, drawers, titlebar, popovers, and other transient control layers. Primary content must remain solid enough for long reading and code/data inspection.

Do not apply glass effects to every panel. Do not place source code or dense evidence on translucent backgrounds.

### 5. One primary action per workflow phase

A Work phase should have one visually dominant CTA. Other actions are secondary, links, disclosures, or contextual tools.

Destructive, paid, write-capable, or otherwise sensitive actions must expose their state and approval requirements before execution rather than relying on dramatic button styling.

### 6. Color is semantic

Use strong color for:

- current focus;
- success;
- warning;
- failure/danger;
- required approval or blocked state.

Do not use color only for decoration. Never make color the only way to distinguish a state; pair it with labels, icons, text, or structure.

The default Hermes palette remains warm graphite/cream with a restrained cognac/amber accent. Alternate themes may change hue, but should preserve the same hierarchy.

### 7. Dense data, calm framing

Developer-facing facts should be compact. Token counts, costs, hashes, paths, model names, and similar technical values may use monospace/tabular numerals where useful.

Keep labels short. Use detail disclosure instead of permanently showing implementation-level explanations.

A good default screen should answer quickly:

1. What am I working on?
2. Where am I in the workflow and what blocks me?
3. What will the worker see, do, and cost?
4. What should I do next?

### 8. Context must be inspectable

Context observability is a first-class RepoDesk product capability. The UI should progressively make it possible to understand:

- what material the worker receives;
- why each source was considered relevant;
- provenance and trust level;
- token cost;
- freshness when available;
- whether it was included or excluded;
- the explicit exclusion reason;
- what changed outside prepared context.

Do not expose raw internal structures when a concise evidence view can answer the same question.

### 9. Motion is feedback, not spectacle

Use short transitions for disclosure, focus changes, and state continuity. A typical UI transition should stay around 120–200 ms.

Avoid looping decorative animation, glowing AI effects, parallax, animated gradients, or motion that delays interaction.

The UI must remain understandable with motion disabled and must honor `prefers-reduced-motion`.

### 10. Accessibility is a baseline

Design for keyboard-first engineering workflows and preserve visible focus states. Command palette and keyboard navigation are product features, not optional polish.

Target WCAG 2.2 AA for functional UI. Text and status must remain understandable without color alone. Preserve the dedicated high-contrast theme and avoid visual effects that weaken it.

### 11. Progressive disclosure over feature dumping

Advanced routing, workers, multi-agent controls, detailed evidence, and configuration should be available without being permanently visible.

Use drawers, details/disclosure, inspectors, dedicated routes, or the command palette based on task frequency and importance.

Hidden must not mean undiscoverable: labels should explain what a disclosure contains.

## Visual anti-patterns

Avoid:

- a dashboard of equally weighted cards;
- full-page gradients;
- glassmorphism on primary content;
- multiple competing accent colors;
- decorative AI sparkles, neon glows, or animated blobs;
- oversized marketing typography inside routine engineering workflows;
- every metric becoming a badge;
- permanent advanced-agent controls on the main Work surface;
- icon-only critical actions without labels or accessible names;
- hover-only information required to understand state;
- animations required to perceive a state transition.

## External design influences

RepoDesk is not intended to clone another product. The current direction borrows principles that are useful for developer tooling:

- **Linear** — focus hierarchy, reduced navigation competition, high information density without equal visual weight everywhere.
- **GitHub Primer** — efficiency, compact interaction patterns, responsive behavior, and accessibility as part of component design.
- **Apple Human Interface Guidelines** — separate navigation/control material from the content layer; use translucency deliberately rather than universally.
- **Radix Themes** — a small token/variant vocabulary that creates consistent hierarchy instead of per-feature styling inventions.
- **Vercel Geist** — restrained surface differentiation and limited use of alternate background levels.
- **Figma Dev Mode** — developer-facing information appears in the context where it is needed instead of turning the entire product into an inspection panel.

These are influences only. RepoDesk's identity comes from its engineering workflow, evidence model, bounded AI context, and warm Hermes visual palette.

## Review checklist for future UI PRs

Before merging UI work, verify:

- Is the active task/action still visually dominant?
- Did this change introduce a new permanent surface that could be contextual instead?
- Is there only one dominant CTA for the current decision?
- Can every state be understood without color alone?
- Does dense data remain readable at normal desktop sizes?
- Are advanced details available without occupying default attention?
- Does reduced-motion mode preserve all information?
- Does the high-contrast theme remain usable?
- Does the change make RepoDesk feel more like an engineering tool rather than a SaaS dashboard?
