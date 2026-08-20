# Venus implementation status

The current branch implements the VirtIO-GPU wire structures and a host-side Venus state machine, but it is **not yet a complete Vulkan/Venus renderer**.

## Implemented

- `CTX_CREATE`, `CTX_DESTROY`, `CTX_ATTACH_RESOURCE`, `CTX_DETACH_RESOURCE` state management.
- Blob resource lifecycle and mapping state.
- Resource UUID response generation.
- Per-ring metadata with globally ordered execution-fence IDs.
- `SUBMIT_3D` parsing including in-fence IDs.
- VirtIO-GPU response construction and protocol-level error classification.

## Required before advertising Venus to guests

1. Populate a real Venus capset from the host renderer; a zero-sized or fabricated capset must not be advertised.
2. Back blob resources with host Vulkan/external memory semantics rather than the in-memory state object alone.
3. Submit Venus command streams to a real Venus renderer backend. The custom Vulkan display renderer in this repository does not decode Venus command streams.
4. Complete asynchronous fence completion and in-fence dependency handling using the renderer's actual fence/event mechanism.
5. Integrate `RESOURCE_CREATE_BLOB`, attach/detach backing, map/unmap and context/resource operations with the renderer backend.
6. Add end-to-end tests that exercise the VirtIO control queue against the renderer backend.

The intended backend boundary is the Mesa `virglrenderer` Venus path. The repository must not claim Venus support merely because protocol structs and a state machine exist.

## CI bootstrap

The branch temporarily applies a source-compatible patch to the published `virglrenderer` 0.1.3 Rust wrapper for Ubuntu 24.04's older generated log-level bindings. This bootstrap step is intended to be removed once the backend uses a version/fork with matching upstream bindings.
