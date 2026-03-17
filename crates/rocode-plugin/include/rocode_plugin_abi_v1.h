#ifndef ROCODE_PLUGIN_ABI_V1_H
#define ROCODE_PLUGIN_ABI_V1_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

#define ROCODE_CABI_VERSION_V1 1u
#define ROCODE_CABI_FLAG_THREADSAFE (1u << 0)

/* Opaque plugin instance handle */
typedef void* rocode_plugin_instance_t;

/*
 * Stable C ABI plugin descriptor (v1).
 *
 * The dynamic library must export:
 *   const rocode_plugin_descriptor_v1_t* rocode_plugin_descriptor_v1(void);
 */
typedef struct rocode_plugin_descriptor_v1 {
  uint32_t abi_version;
  uint32_t flags;
  const char* name;
  const char* version;

  rocode_plugin_instance_t (*create)(void);
  void (*destroy)(rocode_plugin_instance_t instance);

  size_t (*hook_count)(rocode_plugin_instance_t instance);
  const char* (*hook_name)(rocode_plugin_instance_t instance, size_t index);

  /*
   * Invoke a hook.
   * - On success: returns JSON string (allocated by plugin) or NULL for "no change".
   * - On error: writes non-zero to out_code and returns an error message string (allocated by plugin) or NULL.
   */
  char* (*invoke_hook)(
      rocode_plugin_instance_t instance,
      const char* hook,
      const char* input_json,
      const char* output_json,
      int32_t* out_code);

  /* Free a string returned by invoke_hook */
  void (*free_string)(rocode_plugin_instance_t instance, char* s);

  /* Reserved for future expansion (must be zeroed). */
  uintptr_t reserved[8];
} rocode_plugin_descriptor_v1_t;

const rocode_plugin_descriptor_v1_t* rocode_plugin_descriptor_v1(void);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* ROCODE_PLUGIN_ABI_V1_H */
