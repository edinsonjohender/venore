//! Prompt Templates for Context Generation
//!
//! Provides prompt templates for generating V2 context documentation with LLMs.

use std::path::Path;
use crate::context::types::DepthLevel;

/// Builder for context generation prompts
pub struct ContextPromptBuilder;

impl ContextPromptBuilder {
    /// Build prompt with specified depth level
    ///
    /// # Arguments
    /// * `file_path` - Path to the file being analyzed
    /// * `code` - Source code content
    /// * `depth` - Depth level (Minimal, Normal, Detailed, Expert)
    ///
    /// # Example
    /// ```no_run
    /// use venore_core::context::{ContextPromptBuilder, DepthLevel};
    /// use std::path::Path;
    ///
    /// let prompt = ContextPromptBuilder::build_prompt(
    ///     Path::new("src/main.rs"),
    ///     "fn main() {}",
    ///     DepthLevel::Detailed
    /// );
    /// ```
    pub fn build_prompt(file_path: &Path, code: &str, depth: DepthLevel) -> String {
        match depth {
            DepthLevel::Minimal => Self::build_minimal_prompt(file_path, code),
            DepthLevel::Normal => Self::build_normal_prompt(file_path, code),
            DepthLevel::Detailed => Self::build_detailed_prompt(file_path, code),
            DepthLevel::Expert => Self::build_expert_prompt(file_path, code),
        }
    }

    /// Build V2 prompt for comprehensive context generation
    ///
    /// **Note**: This is an alias to `build_prompt(..., DepthLevel::Normal)` for backward compatibility.
    pub fn build_v2_prompt(file_path: &Path, code: &str) -> String {
        Self::build_normal_prompt(file_path, code)
    }

    /// Build normal depth prompt (DEFAULT)
    ///
    /// Target: ~1.5-2K tokens, 1 code snippet (10 lines)
    fn build_normal_prompt(file_path: &Path, code: &str) -> String {
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        format!(
            r#"You are a code documentation expert. Analyze this code and generate comprehensive context documentation.

FILE: {file_name}

CODE:
```
{code}
```

IMPORTANT: Generate markdown content directly. Do NOT wrap your response in code blocks (no ```markdown). Your entire response should be valid markdown that starts with a heading.

Generate your response following this EXACT structure:

# {{Module Name}}

> **Quick Summary**: One-sentence description of what this module does

## Purpose

2-3 paragraphs explaining:
- What problem does this solve?
- Why does this module exist?
- When should you use it?

## API Reference

### Public Methods
```typescript
/**
 * Document all exported functions/methods with JSDoc
 * @param param - Parameter description
 * @returns Return value description
 */
export function methodName(param: Type): ReturnType
```

### Type Definitions
```typescript
// All exported types/interfaces
interface TypeName {{
  field: Type
}}
```

### Events Emitted
| Event | Payload | Description |
|-------|---------|-------------|
| event-name | `{{ field: Type }}` | When this event fires |

## Data Flow & Interactions

### Primary Flow
```
User Action
  ↓ event/method
Component
  ↓ dispatch/call
Store/Service
  ↓ persist/fetch
Database/API
```

### Error Handling Flow
Describe how errors are caught and handled in this module.

## Usage Examples

### Basic Usage
```tsx
import {{ Component }} from './path'

// Working code example showing typical usage
function Example() {{
  return <Component prop={{value}} />
}}
```

### Advanced Patterns
```tsx
// Complex usage patterns, edge cases, or advanced features
```

## Development Guide

### Setup
```bash
# Installation/setup commands
npm install
```

### File Structure
```
module/
├── index.tsx       # Main export
├── components/     # Sub-components
└── hooks/          # Custom hooks
```

### Common Tasks

#### Task: Add new feature
1. Step-by-step instructions
2. Files to modify
3. Tests to add

## Troubleshooting

### Common Issues

#### Issue: "Error message"
**Symptoms**: What the user sees
**Causes**: Root cause explanation
**Solutions**:
```tsx
// Code showing how to fix
```

## Performance & Security

### Benchmarks
| Operation | Latency | Notes |
|-----------|---------|-------|
| load | ~50ms | Average |

### Security Considerations
- List security concerns
- Input validation requirements
- Authentication/authorization needs

## Testing

### Coverage
Overall: X% (estimate based on code analysis)

### Running Tests
```bash
npm run test
```

## Dependencies & Compatibility

### Internal Dependencies
- @/path/to/module - Why this is needed

### External Dependencies
- package@version - Purpose of this dependency

### Browser Compatibility
Chrome, Firefox, Safari (latest versions)

## Notes

### Design Decisions
**Why X over Y?**
Explanation of architectural choices made in this module.

### Future Improvements
- Planned enhancements
- Known limitations to address

---

CRITICAL FORMATTING RULES:
- Your response must be PURE MARKDOWN starting with # heading
- DO NOT wrap your response in ```markdown code blocks
- DO NOT include any backticks before or after your content
- Start directly with: # {{Module Name}}
- End with the Notes section (no closing backticks)

CONTENT GUIDELINES:
- Use markdown formatting throughout (headings, lists, tables, code blocks)
- Include working code examples with proper syntax highlighting
- Be specific and detailed based on actual code analysis
- If information cannot be determined from code, make reasonable inferences
- Use tables for structured data
- Use code blocks with language tags for examples (tsx, typescript, bash, etc.)
- Keep examples practical and realistic
- Focus on clarity and usefulness for developers
"#,
            file_name = file_name,
            code = code
        )
    }

    /// Build detailed depth prompt
    ///
    /// Target: ~3-4K tokens, 3 code snippets (300 chars each)
    fn build_detailed_prompt(file_path: &Path, code: &str) -> String {
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        format!(
            r#"You are a code documentation expert. Analyze this code and generate detailed context documentation with comprehensive explanations and 3 code examples.

FILE: {file_name}

CODE:
```
{code}
```

IMPORTANT: Generate markdown content directly. Do NOT wrap your response in code blocks (no ```markdown). Your entire response should be valid markdown that starts with a heading.

Generate your response following this EXACT structure with DETAILED explanations:

# {{Module Name}}

> **Quick Summary**: One-sentence description of what this module does

## Purpose

3-4 paragraphs with detailed explanation:
- What problem does this solve? (with technical context)
- Why does this module exist? (architectural reasoning)
- When should you use it? (specific use cases)
- How does it fit in the larger system?

## API Reference

### Public Methods
```typescript
/**
 * DETAILED JSDoc with:
 * - Full parameter descriptions with types and constraints
 * - Return value details with possible states
 * - Throws/Error conditions
 * - Usage examples
 * @param param - Detailed parameter description
 * @returns Detailed return description
 * @throws Error conditions
 */
export function methodName(param: Type): ReturnType
```

### Type Definitions
```typescript
// All exported types/interfaces with detailed comments
interface TypeName {{
  field: Type  // Purpose of this field
}}
```

### Events Emitted
| Event | Payload | When | Description |
|-------|---------|------|-------------|
| event-name | `{{ field: Type }}` | Condition | Detailed explanation |

## Data Flow & Interactions

### Primary Flow (Detailed)
```
User Action
  ↓ event/method (with data)
Component (state changes)
  ↓ dispatch/call (payload structure)
Store/Service (processing logic)
  ↓ persist/fetch (data transformation)
Database/API (endpoint details)
```

Explain each step in detail.

### Error Handling Flow
Comprehensive description of error scenarios, recovery mechanisms, and fallback strategies.

## Usage Examples

### Basic Usage
```tsx
import {{ Component }} from './path'

// Working code example showing typical usage
// Include setup, execution, and expected result
function Example() {{
  return <Component prop={{value}} />
}}
```

### Advanced Patterns
```tsx
// Complex usage patterns showing advanced features
// Explain WHY this pattern is useful
// Include edge cases handled
```

### Edge Case Handling
```tsx
// Code showing how to handle edge cases
// Explain common pitfalls and how to avoid them
```

## Development Guide

### Setup
```bash
# Detailed installation/setup commands with explanations
npm install
# Any configuration needed
```

### File Structure
```
module/
├── index.tsx       # Main export (explain organization)
├── components/     # Sub-components (purpose)
├── hooks/          # Custom hooks (when to use)
└── types/          # Type definitions
```

### Common Tasks

#### Task: Add new feature
1. Detailed step-by-step instructions
2. Files to modify with specific locations
3. Code examples for each step
4. Tests to add with examples

#### Task: Debug issues
1. Common debugging approaches
2. Tools to use
3. Key areas to investigate

## Troubleshooting

### Common Issues

#### Issue: "Error message"
**Symptoms**: Detailed description of what the user sees
**Causes**:
- Root cause 1 with technical explanation
- Root cause 2 with conditions
**Solutions**:
```tsx
// Detailed code showing how to fix
// Include explanations of WHY this fixes it
```

#### Issue: "Another common error"
**Symptoms**: ...
**Causes**: ...
**Solutions**: ...

## Performance & Security

### Benchmarks
| Operation | Latency | Throughput | Notes |
|-----------|---------|------------|-------|
| load | ~50ms | 100/s | Conditions affecting performance |

### Optimization Opportunities
- Specific optimization 1 with expected impact
- Specific optimization 2 with implementation hints

### Security Considerations
- Detailed security concern 1 with mitigation strategies
- Input validation requirements with examples
- Authentication/authorization implementation details

## Testing

### Coverage
Overall: X% (estimate based on code analysis)
- Unit tests: Y%
- Integration tests: Z%

### Test Strategy
- What should be tested
- How to write effective tests
- Common test scenarios

### Running Tests
```bash
npm run test
npm run test:coverage
```

## Dependencies & Compatibility

### Internal Dependencies
- @/path/to/module - Detailed explanation of why this is needed and how it's used

### External Dependencies
- package@version - Purpose, usage patterns, and alternatives

### Browser Compatibility
Chrome, Firefox, Safari (latest versions)
Known issues: [list any compatibility concerns]

## Notes

### Design Decisions
**Why X over Y?**
Detailed explanation of architectural choices made in this module with trade-offs analysis.

### Implementation Details
Technical details about the implementation that developers should know.

### Future Improvements
- Planned enhancements with rationale
- Known limitations to address with priority

---

CRITICAL FORMATTING RULES:
- Your response must be PURE MARKDOWN starting with # heading
- DO NOT wrap your response in ```markdown code blocks
- DO NOT include any backticks before or after your content
- Start directly with: # {{Module Name}}
- End with the Notes section (no closing backticks)

CONTENT GUIDELINES:
- Provide detailed description including purpose, key features, and architecture
- Use it to provide specific implementation details with 3 code examples
- Include working code examples with proper syntax highlighting
- Be specific and detailed based on actual code analysis
- Explain WHY things work the way they do, not just WHAT they do
- Use tables for structured data
- Use code blocks with language tags for examples (tsx, typescript, bash, etc.)
- Keep examples practical and realistic with 300 chars max each
"#,
            file_name = file_name,
            code = code
        )
    }

    /// Build expert depth prompt
    ///
    /// Target: ~5-8K tokens, 5 code snippets (500 chars each)
    fn build_expert_prompt(file_path: &Path, code: &str) -> String {
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        format!(
            r#"You are a code documentation expert. Analyze this code and generate comprehensive, expert-level context documentation with line-by-line analysis and 5 detailed code examples.

FILE: {file_name}

CODE:
```
{code}
```

IMPORTANT: Generate markdown content directly. Do NOT wrap your response in code blocks (no ```markdown). Your entire response should be valid markdown that starts with a heading.

Generate your response following this EXACT structure with EXPERT-LEVEL depth:

# {{Module Name}}

> **Quick Summary**: One-sentence description of what this module does

## Purpose

4-5 paragraphs with comprehensive explanation:
- What problem does this solve? (with full technical context and business reasoning)
- Why does this module exist? (complete architectural justification)
- When should you use it? (detailed decision matrix)
- How does it fit in the larger system? (system architecture context)
- What are the trade-offs? (alternative approaches considered)

## Implementation Analysis

### Key Components
Line-by-line breakdown of critical code sections:
- Line X-Y: Explain the purpose and implementation details
- Line Z: Why this approach was chosen over alternatives

### Algorithm Complexity
- Time complexity: O(?)
- Space complexity: O(?)
- Optimization opportunities

## API Reference

### Public Methods
```typescript
/**
 * COMPREHENSIVE JSDoc with:
 * - Full parameter descriptions with types, constraints, and validation rules
 * - Return value details with all possible states
 * - Throws/Error conditions with error codes
 * - Performance characteristics
 * - Thread safety considerations
 * - Usage examples with context
 * @param param - Comprehensive parameter description
 * @returns Comprehensive return description
 * @throws Detailed error conditions
 * @example
 * // Usage example
 */
export function methodName(param: Type): ReturnType
```

### Type Definitions
```typescript
// All exported types/interfaces with comprehensive documentation
interface TypeName {{
  field: Type  // Purpose, constraints, and usage notes
}}
```

### Internal Implementation Details
Technical details about private methods and internal state that developers should understand for maintenance.

### Events Emitted
| Event | Payload | When | Handler | Description |
|-------|---------|------|---------|-------------|
| event-name | `{{ field: Type }}` | Condition | Listener | Comprehensive explanation |

## Data Flow & Interactions

### Primary Flow (Expert Detail)
```
User Action (specific trigger)
  ↓ event/method (data: specific payload structure)
Component (state changes: before/after)
  ↓ dispatch/call (validation: rules applied)
Store/Service (processing logic: step-by-step)
  ↓ persist/fetch (data transformation: schema mapping)
Database/API (endpoint details: request/response contracts)
  ↓ response handling (success/error paths)
UI Update (re-render logic)
```

Explain each step with technical depth including:
- Data structures used
- Validation rules
- Error boundaries
- Performance considerations

### Error Handling Flow
Comprehensive description of:
- Error scenarios with probability and impact
- Recovery mechanisms with code examples
- Fallback strategies with decision logic
- Logging and monitoring integration

### State Management
Detailed explanation of state lifecycle, update patterns, and synchronization.

## Usage Examples

### Basic Usage
```tsx
import {{ Component }} from './path'

// Working code example showing typical usage
// Include setup, execution, and expected result
// Explain every line that's not obvious
function Example() {{
  // Setup explanation
  const value = useSetup()

  // Usage explanation
  return <Component prop={{value}} />
}}
```

### Advanced Patterns
```tsx
// Complex usage patterns showing advanced features
// Detailed explanation of WHY this pattern is optimal
// Include edge cases and their handling
// Show alternative approaches and trade-offs
```

### Edge Case Handling
```tsx
// Comprehensive code showing how to handle edge cases
// Explain common pitfalls and prevention strategies
// Include defensive programming techniques
```

### Performance Optimization
```tsx
// Code showing performance optimizations
// Explain impact and when to apply
// Include benchmarking approach
```

### Testing Example
```tsx
// Complete test example with setup, execution, assertions
// Explain what's being tested and why
// Include both happy path and error cases
```

## Development Guide

### Setup
```bash
# Comprehensive installation/setup commands with explanations
npm install
# Configuration with reasoning
# Environment setup with requirements
```

### File Structure
```
module/
├── index.tsx       # Main export (organization rationale)
├── components/     # Sub-components (architecture decisions)
├── hooks/          # Custom hooks (reusability patterns)
├── utils/          # Helper functions (separation of concerns)
└── types/          # Type definitions (type safety strategy)
```

### Common Tasks

#### Task: Add new feature
1. Comprehensive step-by-step instructions with context
2. Files to modify with specific locations and reasoning
3. Complete code examples for each step with explanations
4. Tests to add with TDD approach
5. Integration considerations
6. Deployment considerations

#### Task: Debug issues
1. Systematic debugging approach
2. Tools and techniques
3. Key areas to investigate with priority
4. Common anti-patterns to look for

#### Task: Refactor for performance
1. Profiling approach
2. Optimization strategies
3. Code examples with before/after
4. Validation approach

## Troubleshooting

### Common Issues

#### Issue: "Error message"
**Symptoms**: Detailed description of what the user sees
**Frequency**: How common (with estimated percentage)
**Impact**: Severity and user impact
**Causes**:
- Root cause 1 with technical deep-dive
- Root cause 2 with conditions and probability
- Contributing factors
**Solutions**:
```tsx
// Comprehensive code showing how to fix
// Include detailed explanations of WHY this fixes it
// Show how to prevent in the future
// Reference specific lines in the original code
```
**Prevention**: How to avoid this issue in future development

#### Issue: "Performance degradation"
**Symptoms**: ...
**Causes**: ...
**Solutions**: ...
**Monitoring**: How to detect early

#### Issue: "Integration failures"
**Symptoms**: ...
**Causes**: ...
**Solutions**: ...

## Performance & Security

### Benchmarks
| Operation | Latency | Throughput | Memory | Notes |
|-----------|---------|------------|--------|-------|
| load | ~50ms | 100/s | 10MB | Specific conditions |
| process | ~200ms | 50/s | 5MB | Peak performance |

### Performance Analysis
- Detailed analysis of performance characteristics
- Bottlenecks identification with profiling data
- Optimization opportunities with expected ROI

### Optimization Guide
1. Specific optimization with code example
   - Expected impact: X% improvement
   - Trade-offs: complexity vs performance
2. Another optimization
   - Implementation difficulty
   - When to apply

### Security Considerations
- Comprehensive security concern 1 with:
  - Threat model
  - Attack vectors
  - Mitigation strategies with code
  - Testing approach
- Input validation requirements with:
  - Validation rules
  - Sanitization approach
  - Examples of invalid inputs
- Authentication/authorization with:
  - Implementation details
  - Best practices
  - Common vulnerabilities

### Security Checklist
- [ ] Item 1 with verification method
- [ ] Item 2 with testing approach

## Testing

### Coverage
Overall: X% (detailed estimate based on code analysis)
- Unit tests: Y% (specific areas covered)
- Integration tests: Z% (integration scenarios)
- E2E tests: W% (user flows)

### Test Strategy
- Comprehensive testing approach with rationale
- What should be tested at each level
- How to write effective tests with examples
- Common test scenarios with priority

### Test Examples
```typescript
// Complete test examples with:
// - Setup/teardown
// - Happy path
// - Error cases
// - Edge cases
// - Performance tests
```

### Running Tests
```bash
npm run test
npm run test:coverage
npm run test:e2e
npm run test:performance
```

## Dependencies & Compatibility

### Internal Dependencies
- @/path/to/module - Comprehensive explanation of:
  - Why this dependency exists
  - How it's used (with code examples)
  - Coupling analysis
  - Alternative options considered

### External Dependencies
- package@version - Complete analysis:
  - Purpose and usage patterns
  - Why this package over alternatives
  - Version constraints and upgrade strategy
  - Known issues
  - License considerations

### Compatibility Matrix
| Environment | Version | Status | Notes |
|-------------|---------|--------|-------|
| Node.js | >= 18 | ✅ | Tested |
| Browser | Modern | ✅ | Chrome, Firefox, Safari |

### Browser Compatibility
Chrome, Firefox, Safari (latest versions)
Detailed known issues:
- Issue 1 with workaround
- Issue 2 with affected versions

## Notes

### Design Decisions
**Why X over Y?**
Comprehensive explanation of architectural choices with:
- Complete trade-offs analysis
- Performance implications
- Maintainability considerations
- Scalability factors
- Team capabilities
- Business constraints

### Implementation Details
Deep technical details about the implementation:
- Algorithm choices with justification
- Data structure decisions
- Concurrency considerations
- Memory management approach

### Historical Context
- Why the code evolved this way
- Previous approaches and lessons learned
- Migration notes from old implementation

### Future Improvements
- Planned enhancements with:
  - Rationale
  - Estimated effort
  - Expected benefits
  - Dependencies
- Known limitations to address with:
  - Priority
  - Impact
  - Proposed solutions

### Related Modules
- Module 1: Relationship and interaction patterns
- Module 2: Integration points

---

CRITICAL FORMATTING RULES:
- Your response must be PURE MARKDOWN starting with # heading
- DO NOT wrap your response in ```markdown code blocks
- DO NOT include any backticks before or after your content
- Start directly with: # {{Module Name}}
- End with the Notes section (no closing backticks)

EXPERT-LEVEL CONTENT GUIDELINES:
- Provide comprehensive description with technical details
- Do NOT assume - cite specific lines from the code
- Analyze implementation line by line for critical sections
- Include 5 detailed code examples (500 chars max each)
- Explain not just WHAT but WHY and HOW
- Include trade-offs, alternatives, and decision rationale
- Reference specific lines and implementation details
- Use tables for structured data
- Use code blocks with language tags for examples
- Provide production-ready examples
- Include monitoring, testing, and operational considerations
"#,
            file_name = file_name,
            code = code
        )
    }

    /// Build minimal prompt for quick context generation
    ///
    /// Target: ~500-800 tokens, no code snippets
    fn build_minimal_prompt(file_path: &Path, code: &str) -> String {
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        format!(
            r#"You are a code documentation expert. Analyze this code and provide a brief, concise context overview.

FILE: {file_name}

CODE:
```
{code}
```

IMPORTANT: Generate markdown content directly. Do NOT wrap your response in code blocks. Start with a heading.

Generate your response with these sections (keep each section brief):

# {{Module Name}}

> **Quick Summary**: One-sentence description

## Purpose

1-2 paragraphs maximum explaining:
- What this module does
- Why it exists
- When to use it

## Key Technologies

Bullet list of main technologies/frameworks used.

## API Overview

List exported functions/classes with brief descriptions (NO code examples):
- `functionName()` - Brief description
- `ClassName` - Brief description

## Dependencies

### Internal
- List internal dependencies

### External
- List external packages

## Notes

Brief notes on:
- Key design decisions
- Known limitations

---

CRITICAL FORMATTING RULES:
- Your response must be PURE MARKDOWN starting with # heading
- DO NOT wrap your response in ```markdown code blocks
- DO NOT include code examples or snippets
- Keep it concise but informative
- Focus on essential information only
- Maximum 500-800 tokens total
"#,
            file_name = file_name,
            code = code
        )
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_build_v2_prompt_includes_code() {
        let path = PathBuf::from("src/components/Button.tsx");
        let code = "export const Button = () => <button>Click</button>";

        let prompt = ContextPromptBuilder::build_v2_prompt(&path, code);

        assert!(prompt.contains(code));
        assert!(prompt.contains("Button.tsx"));
    }

    #[test]
    fn test_build_v2_prompt_has_structure() {
        let path = PathBuf::from("test.ts");
        let code = "const foo = 'bar'";

        let prompt = ContextPromptBuilder::build_v2_prompt(&path, code);

        // Check for required sections
        assert!(prompt.contains("# {Module Name}"));
        assert!(prompt.contains("## Purpose"));
        assert!(prompt.contains("## API Reference"));
        assert!(prompt.contains("## Data Flow & Interactions"));
        assert!(prompt.contains("## Usage Examples"));
        assert!(prompt.contains("## Development Guide"));
        assert!(prompt.contains("## Troubleshooting"));
        assert!(prompt.contains("## Performance & Security"));
        assert!(prompt.contains("## Testing"));
        assert!(prompt.contains("## Dependencies & Compatibility"));
        assert!(prompt.contains("## Notes"));
    }

    #[test]
    fn test_build_v2_prompt_has_guidelines() {
        let path = PathBuf::from("test.ts");
        let code = "const test = true";

        let prompt = ContextPromptBuilder::build_v2_prompt(&path, code);

        assert!(prompt.contains("CONTENT GUIDELINES"));
        assert!(prompt.contains("Use markdown formatting"));
        assert!(prompt.contains("Include working code examples"));
    }

    #[test]
    fn test_build_minimal_prompt_includes_code() {
        let path = PathBuf::from("utils/helper.ts");
        let code = "export function helper() { return true }";

        let prompt = ContextPromptBuilder::build_minimal_prompt(&path, code);

        assert!(prompt.contains(code));
        assert!(prompt.contains("helper.ts"));
    }

    #[test]
    fn test_build_minimal_prompt_is_concise() {
        let path = PathBuf::from("test.ts");
        let code = "const test = true";

        let prompt = ContextPromptBuilder::build_minimal_prompt(&path, code);

        assert!(prompt.contains("Purpose"));
        assert!(prompt.contains("Key Technologies"));
        assert!(prompt.contains("API Overview"));
        assert!(prompt.contains("concise"));
    }

    #[test]
    fn test_handles_file_without_extension() {
        let path = PathBuf::from("Dockerfile");
        let code = "FROM node:18";

        let prompt = ContextPromptBuilder::build_v2_prompt(&path, code);

        assert!(prompt.contains("Dockerfile"));
        assert!(prompt.contains("FROM node:18"));
    }

    #[test]
    fn test_handles_nested_path() {
        let path = PathBuf::from("src/components/ui/Button/index.tsx");
        let code = "export { Button } from './Button'";

        let prompt = ContextPromptBuilder::build_v2_prompt(&path, code);

        assert!(prompt.contains("index.tsx"));
        assert!(prompt.contains("export { Button }"));
    }

    #[test]
    fn test_prompt_length_reasonable() {
        let path = PathBuf::from("test.ts");
        let code = "const x = 1";

        let v2_prompt = ContextPromptBuilder::build_v2_prompt(&path, code);
        let minimal_prompt = ContextPromptBuilder::build_minimal_prompt(&path, code);

        // V2 prompt should be comprehensive
        assert!(v2_prompt.len() > 1000);

        // Minimal prompt should be shorter
        assert!(minimal_prompt.len() < v2_prompt.len());
    }

    // ========================================================================
    // DEPTH LEVEL TESTS
    // ========================================================================

    #[test]
    fn test_depth_level_default() {
        use crate::context::types::DepthLevel;
        assert_eq!(DepthLevel::default(), DepthLevel::Normal);
    }

    #[test]
    fn test_build_prompt_with_minimal() {
        use crate::context::types::DepthLevel;

        let path = PathBuf::from("test.ts");
        let code = "const test = true";

        let prompt = ContextPromptBuilder::build_prompt(&path, code, DepthLevel::Minimal);

        assert!(prompt.contains("brief"));
        assert!(prompt.contains("concise"));
        assert!(prompt.contains("DO NOT include code examples"));
        assert!(prompt.contains("500-800 tokens"));
    }

    #[test]
    fn test_build_prompt_with_normal() {
        use crate::context::types::DepthLevel;

        let path = PathBuf::from("test.ts");
        let code = "const test = true";

        let prompt = ContextPromptBuilder::build_prompt(&path, code, DepthLevel::Normal);

        // Should have all 11 V2 sections
        assert!(prompt.contains("## Purpose"));
        assert!(prompt.contains("## API Reference"));
        assert!(prompt.contains("## Usage Examples"));
        assert!(prompt.contains("Include working code examples"));
    }

    #[test]
    fn test_build_prompt_with_detailed() {
        use crate::context::types::DepthLevel;

        let path = PathBuf::from("test.ts");
        let code = "const test = true";

        let prompt = ContextPromptBuilder::build_prompt(&path, code, DepthLevel::Detailed);

        assert!(prompt.contains("detailed"));
        assert!(prompt.contains("3 code examples"));
        assert!(prompt.contains("DETAILED"));
        assert!(prompt.contains("implementation details"));
        assert!(prompt.contains("300 chars max"));
    }

    #[test]
    fn test_build_prompt_with_expert() {
        use crate::context::types::DepthLevel;

        let path = PathBuf::from("test.ts");
        let code = "const test = true";

        let prompt = ContextPromptBuilder::build_prompt(&path, code, DepthLevel::Expert);

        assert!(prompt.contains("expert"));
        assert!(prompt.contains("5 detailed code examples"));
        assert!(prompt.contains("line-by-line"));
        assert!(prompt.contains("Do NOT assume - cite specific lines"));
        assert!(prompt.contains("EXPERT-LEVEL"));
        assert!(prompt.contains("500 chars max"));
    }

    #[test]
    fn test_prompt_length_increases_with_depth() {
        use crate::context::types::DepthLevel;

        let path = PathBuf::from("test.ts");
        let code = "const x = 1";

        let minimal = ContextPromptBuilder::build_prompt(&path, code, DepthLevel::Minimal);
        let normal = ContextPromptBuilder::build_prompt(&path, code, DepthLevel::Normal);
        let detailed = ContextPromptBuilder::build_prompt(&path, code, DepthLevel::Detailed);
        let expert = ContextPromptBuilder::build_prompt(&path, code, DepthLevel::Expert);

        // Each level should be progressively longer
        assert!(minimal.len() < normal.len());
        assert!(normal.len() < detailed.len());
        assert!(detailed.len() < expert.len());
    }

    #[test]
    fn test_v2_prompt_backward_compatibility() {
        use crate::context::types::DepthLevel;

        let path = PathBuf::from("test.ts");
        let code = "const x = 1";

        // build_v2_prompt should be equivalent to Normal depth
        let v2_prompt = ContextPromptBuilder::build_v2_prompt(&path, code);
        let normal_prompt = ContextPromptBuilder::build_prompt(&path, code, DepthLevel::Normal);

        assert_eq!(v2_prompt, normal_prompt);
    }
}
