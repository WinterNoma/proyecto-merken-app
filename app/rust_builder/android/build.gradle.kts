import javax.inject.Inject
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.Internal
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.TaskAction
import org.gradle.process.ExecOperations

plugins {
    id("com.android.library")
}

abstract class BuildPulsysRust : DefaultTask() {
    @get:Inject
    abstract val execOperations: ExecOperations

    @get:Internal
    abstract val workspaceDirectory: DirectoryProperty

    @get:Internal
    abstract val ndkDirectory: DirectoryProperty

    @get:Internal
    abstract val cargoTargetDirectory: DirectoryProperty

    @get:Input
    abstract val release: Property<Boolean>

    @get:OutputDirectory
    abstract val jniLibsDirectory: DirectoryProperty

    @TaskAction
    fun build() {
        execOperations.exec {
            workingDir(workspaceDirectory.get().asFile)
            environment("ANDROID_NDK_HOME", ndkDirectory.get().asFile.absolutePath)
            environment("CARGO_TARGET_DIR", cargoTargetDirectory.get().asFile.absolutePath)
            commandLine(
                "cargo", "ndk",
                "--target", "arm64-v8a", "--target", "x86_64",
                "--platform", "26", "--output-dir", jniLibsDirectory.get().asFile.absolutePath,
                "build", "--locked", "-p", "pulsys-bridge",
            )
            if (release.get()) {
                args("--release")
            }
        }
    }
}

android {
    namespace = "com.pulsys.rust_builder"
    compileSdk = 37
    ndkVersion = "28.2.13676358"
    defaultConfig {
        minSdk = 26
    }
}

val rustWorkspace = rootProject.layout.projectDirectory.dir("../..")
val rustTarget = layout.buildDirectory.dir("rust-target")

androidComponents {
    val ndk = sdkComponents.ndkDirectory
    onVariants(selector().all()) { variant ->
        val variantName = variant.name.replaceFirstChar(Char::uppercaseChar)
        val rustTask = tasks.register<BuildPulsysRust>("build${variantName}PulsysRust") {
            workspaceDirectory.set(rustWorkspace)
            ndkDirectory.set(ndk)
            cargoTargetDirectory.set(rustTarget)
            release.set(variant.buildType != "debug")
            jniLibsDirectory.set(layout.buildDirectory.dir("generated/jniLibs/${variant.name}"))
            // Cargo tracks the entire workspace and the native toolchain incrementally.
            outputs.upToDateWhen { false }
        }
        checkNotNull(variant.sources.jniLibs)
            .addGeneratedSourceDirectory(rustTask, BuildPulsysRust::jniLibsDirectory)
    }
}
