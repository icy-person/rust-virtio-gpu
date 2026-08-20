# VirtIO-GPU protocol compliance

This project follows the wire protocol defined by the OASIS VirtIO specification and the Linux `virtio_gpu.h` UAPI. The primary reference for the device protocol is VirtIO 1.3, section 5.7.6. The Linux UAPI is used as an implementation cross-check for exact C layouts and command IDs.

## Implemented wire-level surface

| Area | Status |
| --- | --- |
| Control header, little-endian encoding | Implemented |
| 2D resource create / unref | Request structures implemented; device integration in progress |
| Scanout / flush | Existing device path |
| 2D host transfer | Fixed and validated; exact 56-byte request |
| Resource backing attach / detach | Request structures implemented |
| EDID request/response structures | Request structure implemented; response path pending |
| Resource UUID | Request/response structures available |
| Blob create/map/unmap | Existing request structures + validation helpers |
| Scanout blob | Implemented exact 96-byte request structure |
| Context init / Venus context | Existing request structure + validation helpers |
| 3D resource create | Implemented exact 72-byte request structure |
| 3D transfer | Implemented exact 72-byte request structure |
| Submit 3D | Existing 32-byte request structure |
| Cursor update/move | Implemented exact request structures |
| Fence/ring validation | Protocol-level validation helpers |

## Important protocol rules

### Transfer-to-host-2D

The request is:

```text
ctrl_hdr  24
rect      16
offset     8
resource   4
padding    4
----------------
          56 bytes
```

The transfer destination is the resource backing, starting at `offset`. The rectangle must be fully contained by the resource dimensions.

### Blob resources

The implementation distinguishes the three defined memory types:

- `BLOB_MEM_GUEST`
- `BLOB_MEM_HOST3D`
- `BLOB_MEM_HOST3D_GUEST`

Unknown blob memory types and unknown blob flags are rejected. When guest memory participates in the blob, the supplied entries must cover the requested blob size.

### Context initialization

The low 8 bits of `context_init` select the capability set when `VIRTIO_GPU_F_CONTEXT_INIT` is negotiated. Venus contexts therefore use capset ID 4.

### Fences and ring index

A request carrying `VIRTIO_GPU_FLAG_FENCE` must provide a non-zero fence ID. `VIRTIO_GPU_FLAG_INFO_RING_IDX` is only meaningful when context initialization is supported, and ring indexes are restricted to 0..63.

## References

- OASIS VirtIO 1.3, GPU device section: https://docs.oasis-open.org/virtio/virtio/v1.3/virtio-v1.3.html
- Linux `include/uapi/linux/virtio_gpu.h`: https://github.com/torvalds/linux/blob/master/include/uapi/linux/virtio_gpu.h
- Mesa Venus documentation: https://docs.mesa3d.org/drivers/venus.html

The project intentionally keeps the protocol layer independent from the renderer. A complete Venus implementation also needs a renderer capable of consuming the opaque Vulkan command stream and exposing the required external-memory/synchronization semantics; that is a separate execution/backend layer from the wire-format implementation.
