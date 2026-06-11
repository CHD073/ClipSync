package com.clipsync.app;

interface IClipSyncService {
    String getPrimaryClipText() = 1;
    void setPrimaryClipText(String text) = 2;
}
