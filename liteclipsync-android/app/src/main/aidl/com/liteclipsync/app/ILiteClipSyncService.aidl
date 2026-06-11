package com.liteclipsync.app;

interface ILiteClipSyncService {
    String getPrimaryClipText() = 1;
    void setPrimaryClipText(String text) = 2;
}
