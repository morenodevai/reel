#
# Podspec for the Rust FFI plugin (rust_lib_reel).
# Links the static Rust library and required system frameworks.
#
Pod::Spec.new do |s|
  s.name             = 'rust_lib_reel'
  s.version          = '0.0.1'
  s.summary          = 'Reel media organizer Rust core library.'
  s.description      = <<-DESC
Rust core library for Reel, linked via flutter_rust_bridge.
                       DESC
  s.homepage         = 'https://github.com/reel/reel'
  s.license          = { :file => '../LICENSE' }
  s.author           = { 'Reel' => 'dev@reel.com' }

  s.source           = { :path => '.' }
  s.source_files     = 'Classes/**/*'
  s.dependency 'FlutterMacOS'

  s.platform = :osx, '10.15'
  s.swift_version = '5.0'

  # System frameworks required by Rust dependencies:
  # - SystemConfiguration: reqwest proxy detection
  # - Security: native-tls / reqwest HTTPS
  # - CoreFoundation: various system APIs
  # - Metal, MetalPerformanceShaders: llama.cpp GPU acceleration
  s.frameworks = 'SystemConfiguration', 'Security', 'CoreFoundation', 'Metal', 'MetalPerformanceShaders', 'Accelerate'

  # C++ standard library required by llama-cpp-2
  s.libraries = 'c++'

  s.script_phase = {
    :name => 'Build Rust library',
    :script => 'sh "$PODS_TARGET_SRCROOT/../cargokit/build_pod.sh" ../../rust rust_lib_reel',
    :execution_position => :before_compile,
    :input_files => ['${BUILT_PRODUCTS_DIR}/cargokit_phony'],
    :output_files => ["${BUILT_PRODUCTS_DIR}/librust_lib_reel.a"],
  }
  s.pod_target_xcconfig = {
    'DEFINES_MODULE' => 'YES',
    'EXCLUDED_ARCHS[sdk=iphonesimulator*]' => 'i386',
    'OTHER_LDFLAGS' => '-force_load ${BUILT_PRODUCTS_DIR}/librust_lib_reel.a -lc++ -framework SystemConfiguration -framework Security -framework CoreFoundation -framework Metal -framework MetalPerformanceShaders -framework Accelerate',
  }
end
