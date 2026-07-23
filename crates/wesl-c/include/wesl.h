#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

enum WeslManglerKind
#ifdef __cplusplus
  : uint8_t
#endif // __cplusplus
 {
  WESL_MANGLER_ESCAPE = 0,
  WESL_MANGLER_HASH = 1,
  WESL_MANGLER_NONE = 2,
};
#ifndef __cplusplus
typedef uint8_t WeslManglerKind;
#endif // __cplusplus

enum WeslBindingType
#ifdef __cplusplus
  : uint8_t
#endif // __cplusplus
 {
  WESL_BINDING_UNIFORM = 0,
  WESL_BINDING_STORAGE = 1,
  WESL_BINDING_READ_ONLY_STORAGE = 2,
  WESL_BINDING_FILTERING = 3,
  WESL_BINDING_NON_FILTERING = 4,
  WESL_BINDING_COMPARISON = 5,
  WESL_BINDING_FLOAT = 6,
  WESL_BINDING_UNFILTERABLE_FLOAT = 7,
  WESL_BINDING_SINT = 8,
  WESL_BINDING_UINT = 9,
  WESL_BINDING_DEPTH = 10,
  WESL_BINDING_WRITE_ONLY = 11,
  WESL_BINDING_READ_WRITE = 12,
  WESL_BINDING_READ_ONLY = 13,
};
#ifndef __cplusplus
typedef uint8_t WeslBindingType;
#endif // __cplusplus

typedef struct WeslDiagnostic {
  const char *file;
  uintptr_t span_start;
  uintptr_t span_end;
  const char *title;
} WeslDiagnostic;

typedef struct WeslError {
  const char *source;
  const char *message;
  const struct WeslDiagnostic *diagnostics;
  uintptr_t diagnostics_len;
} WeslError;

typedef struct WeslResult {
  bool success;
  const char *data;
  struct WeslError error;
} WeslResult;

typedef struct WeslStringArray {
  const char *const *items;
  uintptr_t len;
} WeslStringArray;

typedef struct WeslBoolMap {
  const char *const *keys;
  const bool *values;
  uintptr_t len;
} WeslBoolMap;

typedef struct WeslCompileOptions {
  bool imports;
  bool condcomp;
  bool generics;
  bool strip;
  bool lower;
  bool validate;
  bool sourcemap;
  WeslManglerKind mangler;
  bool mangle_main;
  struct WeslStringArray keep;
  bool keep_main;
  struct WeslBoolMap features;
  bool naga;
} WeslCompileOptions;

typedef struct WeslResolveSourceResult {
  bool success;
  const char *source;
} WeslResolveSourceResult;

typedef struct WeslResolveSourceResult *(*WeslResolveSourceFunction)(const char *path,
                                                                     void *userdata);

typedef void (*WeslResolveSourceFreeFunction)(const struct WeslResolveSourceResult *result,
                                              void *userdata);

typedef const char *(*WeslResolveStringFunctionOption)(const char *path, void *userdata);

typedef void (*WeslResolveFreeStringFunctionOption)(const char *result, void *userdata);

typedef struct WeslResolverOptions {
  void *userdata;
  WeslResolveSourceFunction resolve_source;
  WeslResolveSourceFreeFunction resolve_source_free;
  WeslResolveStringFunctionOption display_name;
  WeslResolveFreeStringFunctionOption free_display_name;
  WeslResolveStringFunctionOption fs_path;
  WeslResolveFreeStringFunctionOption free_fs_path;
} WeslResolverOptions;

typedef struct WeslBinding {
  uint32_t group;
  uint32_t binding;
  WeslBindingType kind;
  uintptr_t data_len;
  const uint8_t *data;
} WeslBinding;

typedef struct WeslBindingArray {
  const struct WeslBinding *items;
  uintptr_t len;
} WeslBindingArray;

typedef struct WeslExecResult {
  bool success;
  const struct WeslBindingArray *resources;
  struct WeslError error;
} WeslExecResult;

typedef struct WeslStringMap {
  const char *const *keys;
  const char *const *values;
  uintptr_t len;
} WeslStringMap;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * Free with `wesl_free_result`.
 */
struct WeslResult wesl_compile(const char *main,
                               const struct WeslCompileOptions *options,
                               const struct WeslResolverOptions *resolver);

/**
 * Requires the `eval` feature to be enabled.
 *
 * Free with `wesl_free_result`.
 */
struct WeslResult wesl_eval(const char *main,
                            const char *expression,
                            const struct WeslCompileOptions *options,
                            const struct WeslResolverOptions *resolver);

/**
 * Requires the `eval` feature to be enabled.
 *
 * Free with `wesl_free_result`.
 */
struct WeslResult wesl_eval(const char *_main,
                            const char *_expression,
                            const struct WeslCompileOptions *_options,
                            const struct WeslResolverOptions *_resolver);

/**
 * Requires the `eval` feature to be enabled.
 *
 * Free with `wesl_free_exec_result`.
 */
struct WeslExecResult wesl_exec(const char *main,
                                const char *entrypoint,
                                const struct WeslCompileOptions *options,
                                const struct WeslBindingArray *resources,
                                const struct WeslStringMap *overrides,
                                const struct WeslResolverOptions *resolver);

/**
 * Requires the `eval` feature to be enabled.
 *
 * Free with `wesl_free_exec_result`.
 */
struct WeslExecResult wesl_exec(const char *_main,
                                const char *_entrypoint,
                                const struct WeslCompileOptions *_options,
                                const struct WeslBindingArray *_resources,
                                const struct WeslStringMap *_overrides,
                                const struct WeslResolverOptions *_resolver);

void wesl_free_result(struct WeslResult *result);

void wesl_free_exec_result(struct WeslExecResult *result);

/**
 * Note: this function result must not be freed.
 */
const char *wesl_version(void);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus
