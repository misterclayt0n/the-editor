import { MainNativeRuntimeApi } from "../../../apps/electron/src/main/nativeRuntimeApi.js";

export function createTestRuntime(): MainNativeRuntimeApi {
  return new MainNativeRuntimeApi();
}
