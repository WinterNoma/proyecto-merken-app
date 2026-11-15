"""Append pinned models after Gradle compilation, align and sign the final APK.
Streaming avoids large AAR/Gradle asset copies. AWS credentials exist only at build time.
"""
import hashlib,os,shutil,subprocess,sys,zipfile
from pathlib import Path

source=Path(sys.argv[1]);destination=Path(sys.argv[2]);destination.parent.mkdir(exist_ok=True)
if os.environ.get('TRUSTED_MODEL_BUILD')!='true':
    shutil.copyfile(source,destination)
    raise SystemExit(0)
bucket=os.environ['SLM_ASSET_BUCKET'];assert bucket.startswith('merken-slm-assets-')
work=Path(os.environ['RUNNER_TEMP'])/'merken-offline-apk';work.mkdir(exist_ok=True)
unaligned=work/'with-models.apk';aligned=work/'aligned.apk';shutil.copyfile(source,unaligned)
files=[('space-health-base-Q4_K_M.gguf',1107409120,'4fce4f8510d16a058a9e6eef90d6ffd992135259e0fcadd42e69b06d1243dea1'),('wellbeing-v7-lora-f16.gguf',34892576,'521290d5207de280a7fc1ebc7517138e2dd994405347cc451a16fa60d5ffc2ef')]
with zipfile.ZipFile(unaligned,'a',compression=zipfile.ZIP_STORED,allowZip64=True) as apk:
    for name,size,expected in files:
        entry='assets/slm/'+name
        assert entry not in apk.namelist(),'Model was bundled twice'
        info=zipfile.ZipInfo(entry);info.file_size=size;info.compress_type=zipfile.ZIP_STORED
        process=subprocess.Popen(['aws','s3','cp','s3://'+bucket+'/v7/'+name,'-','--only-show-errors','--no-progress'],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
        digest=hashlib.sha256();received=0
        with apk.open(info,'w') as target:
            while True:
                block=process.stdout.read(512*1024)
                if not block:break
                received+=len(block);assert received<=size,'Unexpected model size'
                digest.update(block);target.write(block)
        error=process.stderr.read();assert process.wait()==0,'Private asset download failed'
        assert received==size and digest.hexdigest()==expected,'Model integrity check failed'
tools=Path(os.environ['ANDROID_HOME'])/'build-tools/36.0.0'
subprocess.run([str(tools/'zipalign'),'-f','-P','16','4',str(unaligned),str(aligned)],check=True)
# Only this script's own verified temporary APK is removed to keep runner disk bounded.
assert unaligned.parent.resolve()==work.resolve();unaligned.unlink()
keystore=Path.home()/'.android/debug.keystore'
assert keystore.is_file(),'Gradle debug keystore is missing'
subprocess.run([str(tools/'apksigner'),'sign','--ks',str(keystore),'--ks-key-alias','androiddebugkey','--ks-pass','pass:android','--key-pass','pass:android','--out',str(destination),str(aligned)],check=True)
subprocess.run([str(tools/'apksigner'),'verify',str(destination)],check=True)
with zipfile.ZipFile(destination) as apk:
    for name,_,expected in files:
        with apk.open('assets/slm/'+name) as stream:assert hashlib.file_digest(stream,'sha256').hexdigest()==expected
assert aligned.parent.resolve()==work.resolve();aligned.unlink()
print('Offline model assets embedded, aligned, signed and SHA-256 verified.')
