import {
  instantiateNapiModuleSync as __emnapiInstantiateNapiModuleSync,
  getDefaultContext as __emnapiGetDefaultContext,
  WASI as __WASI,
  createOnMessage as __wasmCreateOnMessageForFsProxy,
} from '@napi-rs/wasm-runtime'
import { memfs } from '@napi-rs/wasm-runtime/fs'
import __wasmUrl from './rolldown-binding.wasm32-wasi.wasm?url'

export const { fs: __fs, vol: __volume } = memfs()

const __wasi = new __WASI({
  version: 'preview1',
  fs: __fs,
  preopens: {
    '/': '/',
  }
})

const __emnapiContext = __emnapiGetDefaultContext()

const __sharedMemory = new WebAssembly.Memory({
  initial: 16384,
  maximum: 65536,
  shared: true,
})

const __wasmFile = await fetch(__wasmUrl).then((res) => res.arrayBuffer())

const {
  instance: __napiInstance,
  module: __wasiModule,
  napiModule: __napiModule,
} = __emnapiInstantiateNapiModuleSync(__wasmFile, {
  context: __emnapiContext,
  asyncWorkPoolSize: 4,
  wasi: __wasi,
  onCreateWorker() {
    const worker = new Worker(new URL('./wasi-worker-browser.mjs', import.meta.url), {
      type: 'module',
    })
    
    worker.addEventListener('message', __wasmCreateOnMessageForFsProxy(__fs))

    return worker
  },
  overwriteImports(importObject) {
    importObject.env = {
      ...importObject.env,
      ...importObject.napi,
      ...importObject.emnapi,
      memory: __sharedMemory,
    }
    return importObject
  },
  beforeInit({ instance }) {
    __napi_rs_initialize_modules(instance)
  },
})

function __napi_rs_initialize_modules(__napiInstance) {
  __napiInstance.exports['__napi_register__Bundler_struct_0']?.()
  __napiInstance.exports['__napi_register__Bundler_impl_5']?.()
  __napiInstance.exports['__napi_register__BindingInputItem_struct_6']?.()
  __napiInstance.exports['__napi_register__BindingResolveOptions_struct_7']?.()
  __napiInstance.exports['__napi_register__BindingTreeshake_struct_8']?.()
  __napiInstance.exports['__napi_register__BindingInputOptions_struct_9']?.()
  __napiInstance.exports['__napi_register__BindingOutputOptions_struct_10']?.()
  __napiInstance.exports['__napi_register__BindingPluginContext_struct_11']?.()
  __napiInstance.exports['__napi_register__BindingPluginContext_impl_15']?.()
  __napiInstance.exports['__napi_register__BindingPluginContextResolvedId_struct_16']?.()
  __napiInstance.exports['__napi_register__BindingPluginOptions_struct_17']?.()
  __napiInstance.exports['__napi_register__BindingPluginWithIndex_struct_18']?.()
  __napiInstance.exports['__napi_register__BindingTransformPluginContext_struct_19']?.()
  __napiInstance.exports['__napi_register__BindingTransformPluginContext_impl_21']?.()
  __napiInstance.exports['__napi_register__BindingAssetSource_struct_22']?.()
  __napiInstance.exports['__napi_register__BindingEmittedAsset_struct_23']?.()
  __napiInstance.exports['__napi_register__BindingHookLoadOutput_struct_24']?.()
  __napiInstance.exports['__napi_register__BindingHookRenderChunkOutput_struct_25']?.()
  __napiInstance.exports['__napi_register__BindingHookResolveIdExtraOptions_struct_26']?.()
  __napiInstance.exports['__napi_register__BindingHookResolveIdOutput_struct_27']?.()
  __napiInstance.exports['__napi_register__BindingHookSideEffects_28']?.()
  __napiInstance.exports['__napi_register__BindingPluginContextResolveOptions_struct_29']?.()
  __napiInstance.exports['__napi_register__BindingBuiltinPlugin_struct_30']?.()
  __napiInstance.exports['__napi_register__BindingBuiltinPluginName_31']?.()
  __napiInstance.exports['__napi_register__ParallelJsPluginRegistry_struct_32']?.()
  __napiInstance.exports['__napi_register__ParallelJsPluginRegistry_impl_34']?.()
  __napiInstance.exports['__napi_register__register_plugins_35']?.()
  __napiInstance.exports['__napi_register__BindingLog_struct_36']?.()
  __napiInstance.exports['__napi_register__BindingLogLevel_37']?.()
  __napiInstance.exports['__napi_register__BindingModuleInfo_struct_38']?.()
  __napiInstance.exports['__napi_register__BindingModuleInfo_impl_40']?.()
  __napiInstance.exports['__napi_register__BindingOutputAsset_struct_41']?.()
  __napiInstance.exports['__napi_register__BindingOutputAsset_impl_45']?.()
  __napiInstance.exports['__napi_register__BindingOutputChunk_struct_46']?.()
  __napiInstance.exports['__napi_register__BindingOutputChunk_impl_64']?.()
  __napiInstance.exports['__napi_register__BindingOutputs_struct_65']?.()
  __napiInstance.exports['__napi_register__BindingOutputs_impl_69']?.()
  __napiInstance.exports['__napi_register__FinalBindingOutputs_struct_70']?.()
  __napiInstance.exports['__napi_register__FinalBindingOutputs_impl_73']?.()
  __napiInstance.exports['__napi_register__RenderedChunk_struct_74']?.()
  __napiInstance.exports['__napi_register__BindingRenderedModule_struct_75']?.()
  __napiInstance.exports['__napi_register__AliasItem_struct_76']?.()
  __napiInstance.exports['__napi_register__BindingSourcemap_struct_77']?.()
  __napiInstance.exports['__napi_register__BindingJSONSourcemap_struct_78']?.()
}
export const BindingLog = __napiModule.exports.BindingLog
export const BindingModuleInfo = __napiModule.exports.BindingModuleInfo
export const BindingOutputAsset = __napiModule.exports.BindingOutputAsset
export const BindingOutputChunk = __napiModule.exports.BindingOutputChunk
export const BindingOutputs = __napiModule.exports.BindingOutputs
export const BindingPluginContext = __napiModule.exports.BindingPluginContext
export const BindingTransformPluginContext = __napiModule.exports.BindingTransformPluginContext
export const Bundler = __napiModule.exports.Bundler
export const FinalBindingOutputs = __napiModule.exports.FinalBindingOutputs
export const ParallelJsPluginRegistry = __napiModule.exports.ParallelJsPluginRegistry
export const BindingBuiltinPluginName = __napiModule.exports.BindingBuiltinPluginName
export const BindingHookSideEffects = __napiModule.exports.BindingHookSideEffects
export const BindingLogLevel = __napiModule.exports.BindingLogLevel
export const registerPlugins = __napiModule.exports.registerPlugins
