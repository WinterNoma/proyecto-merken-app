"""Append the test models to the compiled APK, then align and sign it.

The models come from a private Release of this repository, downloaded into
MODELS_DIR by the workflow (no AWS). Only known files are accepted: each one is
checked by size and SHA-256 while it is streamed into the APK. The base model is
required; the V5 and V7 adapters are optional.
"""
import hashlib, os, shutil, subprocess, sys, zipfile
from pathlib import Path

KNOWN = {
    'space-health-base-Q4_K_M.gguf': (1107409120, '4fce4f8510d16a058a9e6eef90d6ffd992135259e0fcadd42e69b06d1243dea1'),
    'space-health-lora-f16.gguf': (34892576, 'ca3d2b23e39547d0ebafb2e47793b4fba3a2943978e59205328dac47f5dd750f'),
    'wellbeing-v7-lora-f16.gguf': (34892576, '521290d5207de280a7fc1ebc7517138e2dd994405347cc451a16fa60d5ffc2ef'),
    # Dictation (Whisper base int8), installed under stt-whisper-base/.
    'base-encoder.int8.onnx': (29120534, '0b8fb1304b6109976038efff5ace81720e00386f3ff6b54ee8c75291ca0a1e11'),
    'base-decoder.int8.onnx': (130672026, '9759d217388a01b3a4c7c15533201067b48ae819c4daafc8624e64b9409dc02d'),
    'base-tokens.txt': (816730, 'b34b360dbb493e781e479794586d661700670d65564001f23024971d1f2fa126'),
}
SUBDIR = {name: 'stt-whisper-base/' for name in ('base-encoder.int8.onnx', 'base-decoder.int8.onnx', 'base-tokens.txt')}


def entry_for(name):
    return 'assets/slm/' + SUBDIR.get(name, '') + name


BASE = 'space-health-base-Q4_K_M.gguf'

source = Path(sys.argv[1])
destination = Path(sys.argv[2])
destination.parent.mkdir(exist_ok=True)
if os.environ.get('TRUSTED_MODEL_BUILD') != 'true':
    shutil.copyfile(source, destination)
    raise SystemExit(0)

models = Path(os.environ['MODELS_DIR'])
found = sorted(p.name for p in models.iterdir() if p.is_file() and p.name != 'SHA256SUMS.txt')
unknown = [n for n in found if n not in KNOWN]
assert not unknown, f'Unknown model files: {unknown}'
assert BASE in found, 'The base model was not downloaded'

work = Path(os.environ['RUNNER_TEMP']) / 'merken-offline-apk'
work.mkdir(exist_ok=True)
unaligned = work / 'with-models.apk'
aligned = work / 'aligned.apk'
shutil.copyfile(source, unaligned)
with zipfile.ZipFile(unaligned, 'a', compression=zipfile.ZIP_STORED, allowZip64=True) as apk:
    for name in found:
        size, expected = KNOWN[name]
        entry = entry_for(name)
        assert entry not in apk.namelist(), 'Model was bundled twice'
        info = zipfile.ZipInfo(entry)
        info.file_size = size
        info.compress_type = zipfile.ZIP_STORED
        digest = hashlib.sha256()
        received = 0
        with (models / name).open('rb') as src, apk.open(info, 'w') as target:
            while block := src.read(512 * 1024):
                received += len(block)
                assert received <= size, 'Unexpected model size'
                digest.update(block)
                target.write(block)
        assert received == size and digest.hexdigest() == expected, f'Model integrity check failed: {name}'
        print(f'Embedded {name}')

tools = Path(os.environ['ANDROID_HOME']) / 'build-tools/36.0.0'
subprocess.run([str(tools / 'zipalign'), '-f', '-P', '16', '4', str(unaligned), str(aligned)], check=True)
unaligned.unlink()

# The CI runner may not have Gradle's debug key: create a throwaway debug key.
keystore = Path.home() / '.android/debug.keystore'
if not keystore.is_file():
    keystore.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(['keytool', '-genkeypair', '-keystore', str(keystore), '-storepass', 'android',
                    '-alias', 'androiddebugkey', '-keypass', 'android', '-keyalg', 'RSA', '-keysize', '2048',
                    '-validity', '10000', '-dname', 'CN=Android Debug,O=Android,C=US'], check=True)
subprocess.run([str(tools / 'apksigner'), 'sign', '--ks', str(keystore), '--ks-key-alias', 'androiddebugkey',
                '--ks-pass', 'pass:android', '--key-pass', 'pass:android', '--out', str(destination), str(aligned)],
               check=True)
subprocess.run([str(tools / 'apksigner'), 'verify', str(destination)], check=True)
with zipfile.ZipFile(destination) as apk:
    for name in found:
        with apk.open(entry_for(name)) as stream:
            assert hashlib.file_digest(stream, 'sha256').hexdigest() == KNOWN[name][1]
aligned.unlink()
print('Models embedded, aligned, signed and SHA-256 verified: ' + ', '.join(found))
