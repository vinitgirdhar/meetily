'use client';

import { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranscripts } from '@/contexts/TranscriptContext';
import { useRecordingState } from '@/contexts/RecordingStateContext';

const DEBOUNCE_MS = 2500; // wait for a pause in speech before asking
const CONTEXT_LINES = 12; // how many recent transcript lines to send

export interface LiveCopilotState {
  answer: string;
  isLoading: boolean;
  error: string | null;
  enabled: boolean;
  toggle: () => void;
  askNow: () => void;
}

/**
 * Live meeting copilot. When enabled during a recording, it watches the
 * transcript and — after a short pause in new speech — asks the backend to
 * suggest an answer to the latest question, using the user's resume as context.
 *
 * ponytail: debounced polling on transcript change; upgrade to streaming if the
 * backend ever exposes a token stream.
 */
export function useLiveCopilot(): LiveCopilotState {
  const { transcripts } = useTranscripts();
  const { isRecording } = useRecordingState();

  const [answer, setAnswer] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [enabled, setEnabled] = useState(false);

  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastAskedTextRef = useRef<string>('');
  const inFlightRef = useRef(false);

  const buildContext = useCallback((): string => {
    return transcripts
      .slice(-CONTEXT_LINES)
      .map((t) => t.text)
      .join('\n')
      .trim();
  }, [transcripts]);

  const runSuggest = useCallback(async () => {
    const context = buildContext();
    if (!context || context === lastAskedTextRef.current) return;
    if (inFlightRef.current) return;

    inFlightRef.current = true;
    lastAskedTextRef.current = context;
    setIsLoading(true);
    setError(null);
    try {
      const result = await invoke<string>('copilot_suggest_answer', { transcript: context });
      const trimmed = (result || '').trim();
      if (trimmed) setAnswer(trimmed);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      inFlightRef.current = false;
      setIsLoading(false);
    }
  }, [buildContext]);

  // Auto-trigger on transcript changes while enabled + recording, debounced.
  useEffect(() => {
    if (!enabled || !isRecording) return;
    if (transcripts.length === 0) return;

    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      runSuggest();
    }, DEBOUNCE_MS);

    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [transcripts, enabled, isRecording, runSuggest]);

  const toggle = useCallback(() => {
    setEnabled((prev) => {
      const next = !prev;
      if (!next) {
        // Turning off: clear pending work.
        if (debounceRef.current) clearTimeout(debounceRef.current);
      }
      return next;
    });
  }, []);

  // Manual trigger ignores the "same text" guard so the user can force a retry.
  const askNow = useCallback(() => {
    lastAskedTextRef.current = '';
    runSuggest();
  }, [runSuggest]);

  return { answer, isLoading, error, enabled, toggle, askNow };
}
