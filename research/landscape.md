# Landscape: agent-generated UI (October 2026)

Working name for this project: **Manifest**. These notes are what spec v0 positions against.

## TL;DR

The **write** half of the idea (agents emit declarative UI, clients render natively, no executable code) is already solved and standardizing, mainly through **A2UI**. The **read** half (the model perceiving the rendered UI losslessly and cheaply) and **lifecycle** (ephemeral / TTL / persistent views) are not covered by any spec we found.

So Manifest should **not** compete with A2UI on component generation. It should:
1. Specify **perception**: a layout-resolved, levelled, diffable read-back of the rendered tree.
2. Specify **lifecycle**: views that dissolve on their own terms.
3. Offer a **compact text syntax** that is cheaper in tokens than JSON and round-trips losslessly to an A2UI-compatible node model.

## Existing work

### A2UI (Google): the closest overlap
- Launched December 2025 under Apache 2.0. v0.9 released April 2026. A v1.0 was reportedly targeted for Q4 2026, but that is unconfirmed.
- Server → client messages: `createSurface`, `updateComponents`, `updateDataModel`, `deleteSurface`.
- Components form a **flat adjacency list keyed by `id`**, with exactly one `root`. Children are referenced by ID (`children`, `child`) or by a data-bound template (`{path, componentId}`).
- The data model is plain JSON, bound by JSON Pointer. `updateDataModel` uses upsert/remove semantics at a path.
- Client → server messages: `action` (`name`, `surfaceId`, `sourceComponentId`, `timestamp`, `context`) and `error`. With `sendDataModel: true`, the whole data model rides along on every message. Typing alone sends nothing.
- Catalog negotiation (`supportedCatalogIds`), client-side validation `checks`, prompt-first design, streaming (JSONL), and transports over A2A, AG-UI, MCP and WebSockets.
- **Gaps:** no perception of rendered layout or visual state, no TTL or ephemeral lifecycle (surfaces live until `deleteSurface`), no accessibility guidance, and verbose JSON.

### MCP Apps (SEP-1865): the HTML route
- An official optional MCP extension (`io.modelcontextprotocol/ui`), with a stable spec dated 2026-01-26. Hosts including Claude, ChatGPT, Goose and VS Code have shipped support.
- UI is **HTML in a sandboxed iframe** (`text/html;profile=mcp-app`), predeclared as `ui://` resources. JSON-RPC runs over `postMessage`.
- The model perceives the view only through **`ui/update-model-context`**: free-form context that the view pushes, which overwrites the previous push and may be deferred until the next user turn.
- Lifecycle is `ui/resource-teardown`, plus display modes `inline`, `fullscreen` and `pip`. Other content types are "reserved for future extensions", which leaves an opening for a declarative format.

### Vercel json-render
- Open source, launched January 2026. Developers declare a catalog with Zod schemas, and the model emits JSON constrained to it.
- Library-level, not a wire protocol. Same write-only shape as A2UI.

### AG-UI / CopilotKit
- A bidirectional event runtime between agents and frontends. CopilotKit groups generative UI into three patterns: controlled (AG-UI), declarative (A2UI / Open-JSON-UI) and open-ended (MCP Apps).
- AG-UI is a **transport** Manifest could ride on, not a competitor.

### Agent perception research
- **OSWorld** compares accessibility-tree input with screenshots. Raw trees run to millions of tokens, so the authors prune them to `tag, name, text, position, size`, which is close to what Manifest's perception format would emit natively.
- **DailyDroid** (2026 preprint): text-only UI trees performed about as well as text plus screenshot, with screenshots giving marginally better results.
- **WebCanvas** reports that screenshots help with layout and with elements the accessibility tree misses.
- Known accessibility-tree failure modes include canvas and custom UIs with no entries, unlabeled icon buttons, and off-screen or modal-hidden elements inflating context. **All of these vanish when the tree is the source of truth instead of an after-the-fact export.** This is Manifest's core argument.

## Positioning

| | A2UI | MCP Apps | Manifest (proposed) |
|---|---|---|---|
| Model writes UI | JSON components | HTML/JS | Compact text, A2UI-compatible nodes |
| Executable code | No | Yes (sandboxed) | No |
| Model reads UI back | Data model on action only | Free-form context push | **Levelled, layout-resolved tree + diffs** |
| Lifecycle | Explicit delete | Teardown | **ephemeral / ttl / until / persist** |
| Spatial info for model | None | None | **Computed boxes** |
| Accessibility | Unspecified | Whatever the HTML does | **Roles and labels required by schema** |

## Strategic recommendation

- Treat A2UI as the **interop target**. Manifest nodes should map 1:1 to A2UI components where possible, so a Manifest renderer can also consume A2UI, and Manifest can be proposed to the A2UI project as a perception and lifecycle extension.
- Ship as an **MCP server and tool format** first, since it works with today's models with no lab buy-in.
- Lead with the **benchmark**: tokens to create, tokens to perceive, update cost, and QA accuracy versus HTML plus screenshot.

## Sources
- [Introducing A2UI (Google Developers Blog)](https://developers.googleblog.com/introducing-a2ui-an-open-project-for-agent-driven-interfaces/)
- [A2UI v0.9 spec](https://a2ui.org/specification/v0.9-a2ui/)
- [A2UI v0.9: What's new (CopilotKit)](https://www.copilotkit.ai/blog/a2ui-whats-new-in-google-generative-ui-spec)
- [A2UI introduction (a2ui.sh)](https://a2ui.sh/articles/introduction-to-a2ui)
- [MCP Apps spec 2026-01-26](https://github.com/modelcontextprotocol/ext-apps/blob/main/specification/2026-01-26/apps.mdx)
- [SEP-1865](https://modelcontextprotocol.io/seps/1865-mcp-apps-interactive-user-interfaces-for-mcp)
- [pydantic-ai MCP Apps issue (host support list)](https://github.com/pydantic/pydantic-ai/issues/6613)
- [Vercel json-render (InfoQ)](https://www.infoq.com/news/2026/03/vercel-json-render)
- [CopilotKit generative UI patterns](https://context7.com/copilotkit/generative-ui)
- [OSWorld](https://arxiv.org/pdf/2404.07972)
- [WebCanvas](https://arxiv.org/pdf/2406.12373)
- [DailyDroid / "Do LLMs need to see everything?"](https://www.alphaxiv.org/abs/2604.17817)
