'use client';

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { toast } from 'sonner';
import { Button } from './ui/button';
import { FileText, Upload, Trash2, RefreshCw } from 'lucide-react';

interface ResumeContext {
  content: string;
  filename: string | null;
}

/**
 * Lets the user attach a resume (PDF / .txt / .md) as background context.
 * The extracted text is fed into the summary LLM so it can answer questions
 * raised during a meeting using the user's own background.
 *
 * Single active context: uploading replaces the previous one.
 */
export function ResumeContextSettings() {
  const [resume, setResume] = useState<ResumeContext | null>(null);
  const [isUploading, setIsUploading] = useState(false);

  const loadResume = useCallback(async () => {
    try {
      const data = await invoke<ResumeContext | null>('api_get_resume_context');
      setResume(data);
    } catch (err) {
      console.error('Failed to load resume context:', err);
    }
  }, []);

  useEffect(() => {
    loadResume();
  }, [loadResume]);

  const handleUpload = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: 'Resume', extensions: ['pdf', 'txt', 'md', 'markdown'] }],
      });
      if (!selected || typeof selected !== 'string') return; // cancelled

      setIsUploading(true);
      const content = await invoke<string>('api_extract_resume_text', { filePath: selected });

      const filename = selected.split(/[\\/]/).pop() || null;
      await invoke('api_save_resume_context', { content, filename });

      setResume({ content, filename });
      toast.success('Resume attached — it will be used as context for summaries');
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      toast.error(msg);
    } finally {
      setIsUploading(false);
    }
  };

  const handleClear = async () => {
    try {
      await invoke('api_clear_resume_context');
      setResume(null);
      toast.success('Resume context removed');
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      toast.error(msg);
    }
  };

  return (
    <div className="bg-white rounded-lg border border-gray-200 p-6 shadow-sm">
      <h3 className="text-lg font-semibold mb-2">Meeting Context (Resume)</h3>
      <p className="text-sm text-gray-600 mb-4">
        Attach your resume (PDF, .txt, or .md). Its text is given to the AI as background so
        summaries can answer questions about your experience raised during the meeting.
      </p>

      {resume ? (
        <div className="flex items-center justify-between gap-3 rounded-md border bg-muted/40 p-3">
          <div className="flex items-center gap-2 min-w-0">
            <FileText className="h-5 w-5 shrink-0 text-blue-600" />
            <div className="min-w-0">
              <div className="truncate text-sm font-medium">
                {resume.filename || 'Attached resume'}
              </div>
              <div className="text-xs text-muted-foreground">
                {resume.content.length.toLocaleString()} characters of context
              </div>
            </div>
          </div>
          <div className="flex items-center gap-2 shrink-0">
            <Button variant="outline" size="sm" onClick={handleUpload} disabled={isUploading}>
              {isUploading ? (
                <RefreshCw className="h-4 w-4 animate-spin" />
              ) : (
                <Upload className="h-4 w-4" />
              )}
            </Button>
            <Button variant="ghost" size="sm" onClick={handleClear} title="Remove resume context">
              <Trash2 className="h-4 w-4 text-red-500" />
            </Button>
          </div>
        </div>
      ) : (
        <Button variant="outline" onClick={handleUpload} disabled={isUploading} className="w-full">
          {isUploading ? (
            <>
              <RefreshCw className="mr-2 h-4 w-4 animate-spin" />
              Reading file...
            </>
          ) : (
            <>
              <Upload className="mr-2 h-4 w-4" />
              Upload Resume
            </>
          )}
        </Button>
      )}
    </div>
  );
}
