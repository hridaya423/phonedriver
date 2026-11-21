/*
 * Parts of this file are adapted from scrcpy (com.genymobile.scrcpy.opengl.OpenGLRunner and AffineOpenGLFilter),
 * Copyright (C) 2018 Genymobile, Copyright (C) 2018-2026 Romain Vimont, licensed under the Apache License 2.0.
 */
package dev.phonedriver.agent;

import android.graphics.SurfaceTexture;
import android.opengl.EGL14;
import android.opengl.EGLConfig;
import android.opengl.EGLContext;
import android.opengl.EGLDisplay;
import android.opengl.EGLExt;
import android.opengl.EGLSurface;
import android.opengl.GLES11Ext;
import android.opengl.GLES20;
import android.os.Build;
import android.os.Handler;
import android.os.HandlerThread;
import android.view.Surface;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.FloatBuffer;
import java.util.concurrent.CountDownLatch;

/**
 * Copies every frame of a virtual display to the encoder's input surface through OpenGL.
 *
 * <p>When a virtual display renders straight into an encoder's surface, SurfaceFlinger may hand that display to the
 * hardware composer. On the Galaxy A53 (Exynos 1280, One UI 8, Android 16) the composer then fails every frame
 * ("deliverWinConfigData failed: Invalid argument") and the encoder receives nothing, at any size. A display whose
 * consumer is a SurfaceTexture is composed by the GPU instead, which works; this class is that SurfaceTexture.
 */
final class GlRelay {
    private static final String VERTEX = ""
            + "attribute vec4 aPosition;\n"
            + "attribute vec4 aTexCoord;\n"
            + "uniform mat4 uTexMatrix;\n"
            + "varying vec2 vTexCoord;\n"
            + "void main() {\n"
            + "    gl_Position = aPosition;\n"
            + "    vTexCoord = (uTexMatrix * aTexCoord).xy;\n"
            + "}\n";
    private static final String FRAGMENT = ""
            + "#extension GL_OES_EGL_image_external : require\n"
            + "precision highp float;\n"
            + "uniform samplerExternalOES uTexture;\n"
            + "varying vec2 vTexCoord;\n"
            + "void main() {\n"
            + "    gl_FragColor = texture2D(uTexture, vTexCoord);\n"
            + "}\n";
    private static final float[] QUAD = {-1, -1, 1, -1, -1, 1, 1, 1};
    private static final float[] TEX = {0, 0, 1, 0, 0, 1, 1, 1};

    private static HandlerThread thread;
    private static Handler handler;
    // Relays on this thread that initialized the default display. eglGetDisplay returns the same display to each,
    // and eglTerminate would free every relay's context on it, so only the last one to go terminates it.
    private static int displayUsers;

    private final int width;
    private final int height;
    private EGLDisplay eglDisplay = EGL14.EGL_NO_DISPLAY;
    private EGLContext eglContext = EGL14.EGL_NO_CONTEXT;
    private EGLSurface eglSurface = EGL14.EGL_NO_SURFACE;
    private boolean displayHeld;
    private SurfaceTexture texture;
    private Surface input;
    private int textureId;
    private int program;
    private int aPosition;
    private int aTexCoord;
    private int uTexMatrix;
    private final float[] matrix = new float[16];
    private final FloatBuffer quad = floats(QUAD);
    private final FloatBuffer tex = floats(TEX);
    private volatile boolean stopped;

    private GlRelay(int width, int height) {
        this.width = width;
        this.height = height;
    }

    // `setprop debug.phonedriver.gl 1` forces the relay on and `0` off.
    static boolean wanted() {
        String forced = property("debug.phonedriver.gl");
        if ("1".equals(forced)) {
            return true;
        }
        if ("0".equals(forced)) {
            return false;
        }
        return "samsung".equalsIgnoreCase(Build.MANUFACTURER);
    }

    private static String property(String key) {
        try {
            Class<?> c = Class.forName("android.os.SystemProperties");
            return (String) c.getMethod("get", String.class, String.class).invoke(null, key, "");
        } catch (ReflectiveOperationException | RuntimeException e) {
            return "";
        }
    }

    static GlRelay start(int width, int height, Surface output) throws Exception {
        synchronized (GlRelay.class) {
            if (thread == null) {
                thread = new HandlerThread("phonedriver-gl");
                thread.start();
                handler = new Handler(thread.getLooper());
            }
        }
        GlRelay relay = new GlRelay(width, height);
        Exception[] failure = new Exception[1];
        CountDownLatch done = new CountDownLatch(1);
        handler.post(() -> {
            try {
                relay.init(output);
            } catch (Exception e) {
                failure[0] = e;
                relay.destroy();
            } finally {
                done.countDown();
            }
        });
        done.await();
        if (failure[0] != null) {
            throw failure[0];
        }
        System.err.println("video: OpenGL relay " + width + "x" + height);
        return relay;
    }

    Surface input() {
        return input;
    }

    private void init(Surface output) {
        eglDisplay = EGL14.eglGetDisplay(EGL14.EGL_DEFAULT_DISPLAY);
        int[] version = new int[2];
        if (eglDisplay == EGL14.EGL_NO_DISPLAY || !EGL14.eglInitialize(eglDisplay, version, 0, version, 1)) {
            throw new IllegalStateException("EGL: no display");
        }
        displayHeld = true;
        displayUsers++;
        int[] attribs = {
            EGL14.EGL_RED_SIZE, 8, EGL14.EGL_GREEN_SIZE, 8, EGL14.EGL_BLUE_SIZE, 8, EGL14.EGL_ALPHA_SIZE, 8,
            EGL14.EGL_RENDERABLE_TYPE, EGL14.EGL_OPENGL_ES2_BIT, EGL14.EGL_NONE
        };
        EGLConfig[] configs = new EGLConfig[1];
        int[] count = new int[1];
        EGL14.eglChooseConfig(eglDisplay, attribs, 0, configs, 0, 1, count, 0);
        if (count[0] <= 0) {
            throw new IllegalStateException("EGL: no ES2 config");
        }
        eglContext = EGL14.eglCreateContext(eglDisplay, configs[0], EGL14.EGL_NO_CONTEXT,
                new int[] {EGL14.EGL_CONTEXT_CLIENT_VERSION, 2, EGL14.EGL_NONE}, 0);
        if (eglContext == null || eglContext == EGL14.EGL_NO_CONTEXT) {
            throw new IllegalStateException("EGL: no context");
        }
        eglSurface = EGL14.eglCreateWindowSurface(eglDisplay, configs[0], output, new int[] {EGL14.EGL_NONE}, 0);
        if (eglSurface == null || eglSurface == EGL14.EGL_NO_SURFACE) {
            throw new IllegalStateException("EGL: no window surface on the encoder");
        }
        if (!EGL14.eglMakeCurrent(eglDisplay, eglSurface, eglSurface, eglContext)) {
            throw new IllegalStateException("EGL: make current failed");
        }
        program = link(compile(GLES20.GL_VERTEX_SHADER, VERTEX), compile(GLES20.GL_FRAGMENT_SHADER, FRAGMENT));
        aPosition = GLES20.glGetAttribLocation(program, "aPosition");
        aTexCoord = GLES20.glGetAttribLocation(program, "aTexCoord");
        uTexMatrix = GLES20.glGetUniformLocation(program, "uTexMatrix");

        int[] ids = new int[1];
        GLES20.glGenTextures(1, ids, 0);
        textureId = ids[0];
        GLES20.glBindTexture(GLES11Ext.GL_TEXTURE_EXTERNAL_OES, textureId);
        GLES20.glTexParameteri(GLES11Ext.GL_TEXTURE_EXTERNAL_OES, GLES20.GL_TEXTURE_MIN_FILTER, GLES20.GL_LINEAR);
        GLES20.glTexParameteri(GLES11Ext.GL_TEXTURE_EXTERNAL_OES, GLES20.GL_TEXTURE_MAG_FILTER, GLES20.GL_LINEAR);
        GLES20.glTexParameteri(GLES11Ext.GL_TEXTURE_EXTERNAL_OES, GLES20.GL_TEXTURE_WRAP_S, GLES20.GL_CLAMP_TO_EDGE);
        GLES20.glTexParameteri(GLES11Ext.GL_TEXTURE_EXTERNAL_OES, GLES20.GL_TEXTURE_WRAP_T, GLES20.GL_CLAMP_TO_EDGE);
        check("texture");

        texture = new SurfaceTexture(textureId);
        texture.setDefaultBufferSize(width, height);
        input = new Surface(texture);
        // The listener runs on this handler's thread, where the context is current.
        texture.setOnFrameAvailableListener(t -> render(), handler);
    }

    private void render() {
        if (stopped) {
            return;
        }
        try {
            draw();
        } catch (RuntimeException e) {
            // A throw here would end the agent: this runs on a looper thread. The encoder may already be stopping.
            stopped = true;
            System.err.println("video: OpenGL relay stopped: " + e);
        }
    }

    // Another relay on this thread (the main stream and a hidden display's) makes its own context current, so each
    // draw binds this relay's context before it touches the texture.
    private void draw() {
        if (!EGL14.eglMakeCurrent(eglDisplay, eglSurface, eglSurface, eglContext)) {
            throw new IllegalStateException("EGL: make current failed: 0x" + Integer.toHexString(EGL14.eglGetError()));
        }
        texture.updateTexImage();
        texture.getTransformMatrix(matrix);
        GLES20.glViewport(0, 0, width, height);
        GLES20.glUseProgram(program);
        GLES20.glActiveTexture(GLES20.GL_TEXTURE0);
        GLES20.glBindTexture(GLES11Ext.GL_TEXTURE_EXTERNAL_OES, textureId);
        GLES20.glUniformMatrix4fv(uTexMatrix, 1, false, matrix, 0);
        GLES20.glEnableVertexAttribArray(aPosition);
        GLES20.glVertexAttribPointer(aPosition, 2, GLES20.GL_FLOAT, false, 0, quad);
        GLES20.glEnableVertexAttribArray(aTexCoord);
        GLES20.glVertexAttribPointer(aTexCoord, 2, GLES20.GL_FLOAT, false, 0, tex);
        GLES20.glDrawArrays(GLES20.GL_TRIANGLE_STRIP, 0, 4);
        // Keep the display's own timestamp (System.nanoTime's clock): Video times its barriers by it.
        EGLExt.eglPresentationTimeANDROID(eglDisplay, eglSurface, texture.getTimestamp());
        EGL14.eglSwapBuffers(eglDisplay, eglSurface);
    }

    // Before the encoder's surface is released: the relay draws into it.
    void release() {
        CountDownLatch done = new CountDownLatch(1);
        handler.post(() -> {
            stopped = true;
            destroy();
            done.countDown();
        });
        try {
            done.await();
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
    }

    private void destroy() {
        if (texture != null) {
            texture.setOnFrameAvailableListener(null);
        }
        if (eglDisplay != EGL14.EGL_NO_DISPLAY) {
            boolean current = eglContext != EGL14.EGL_NO_CONTEXT
                    && eglSurface != EGL14.EGL_NO_SURFACE
                    && EGL14.eglMakeCurrent(eglDisplay, eglSurface, eglSurface, eglContext);
            if (current && textureId != 0) {
                GLES20.glDeleteTextures(1, new int[] {textureId}, 0);
            }
            if (current && program != 0) {
                GLES20.glDeleteProgram(program);
            }
            EGL14.eglMakeCurrent(eglDisplay, EGL14.EGL_NO_SURFACE, EGL14.EGL_NO_SURFACE, EGL14.EGL_NO_CONTEXT);
            if (eglSurface != EGL14.EGL_NO_SURFACE) {
                EGL14.eglDestroySurface(eglDisplay, eglSurface);
            }
            if (eglContext != EGL14.EGL_NO_CONTEXT) {
                EGL14.eglDestroyContext(eglDisplay, eglContext);
            }
            if (displayHeld) {
                displayHeld = false;
                if (--displayUsers == 0) {
                    EGL14.eglTerminate(eglDisplay);
                }
            }
        }
        eglDisplay = EGL14.EGL_NO_DISPLAY;
        eglContext = EGL14.EGL_NO_CONTEXT;
        eglSurface = EGL14.EGL_NO_SURFACE;
        if (texture != null) {
            texture.release();
        }
        if (input != null) {
            input.release();
        }
    }

    private static int compile(int type, String source) {
        int shader = GLES20.glCreateShader(type);
        GLES20.glShaderSource(shader, source);
        GLES20.glCompileShader(shader);
        int[] ok = new int[1];
        GLES20.glGetShaderiv(shader, GLES20.GL_COMPILE_STATUS, ok, 0);
        if (ok[0] == 0) {
            String log = GLES20.glGetShaderInfoLog(shader);
            GLES20.glDeleteShader(shader);
            throw new IllegalStateException("GL: shader did not compile: " + log);
        }
        return shader;
    }

    private static int link(int vertex, int fragment) {
        int p = GLES20.glCreateProgram();
        GLES20.glAttachShader(p, vertex);
        GLES20.glAttachShader(p, fragment);
        GLES20.glLinkProgram(p);
        GLES20.glDeleteShader(vertex);
        GLES20.glDeleteShader(fragment);
        int[] ok = new int[1];
        GLES20.glGetProgramiv(p, GLES20.GL_LINK_STATUS, ok, 0);
        if (ok[0] == 0) {
            String log = GLES20.glGetProgramInfoLog(p);
            GLES20.glDeleteProgram(p);
            throw new IllegalStateException("GL: program did not link: " + log);
        }
        return p;
    }

    private static void check(String what) {
        int e = GLES20.glGetError();
        if (e != GLES20.GL_NO_ERROR) {
            throw new IllegalStateException("GL error 0x" + Integer.toHexString(e) + " at " + what);
        }
    }

    private static FloatBuffer floats(float[] values) {
        FloatBuffer b = ByteBuffer.allocateDirect(values.length * 4).order(ByteOrder.nativeOrder()).asFloatBuffer();
        b.put(values).position(0);
        return b;
    }
}
