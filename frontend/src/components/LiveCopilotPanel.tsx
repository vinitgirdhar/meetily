'use client';

import { Button } from '@/components/ui/button';
import { Sparkles, RefreshCw, X } from 'lucide-react';
import { useLiveCopilot } from '@/hooks/useLiveCopilot';
import { useRecordingState } from '@/contexts/RecordingStateContext';

/**
 * Floating live-copilot panel shown during recording. Toggle it on and it
 * suggests answers to questions as they come up in the meeting, grounded in
 * the user's uploaded resume context.
 */
export function LiveCopilotPanel() {
  const { isRecording } = useRecordingState();
  const { answer, isLoading, error, enabled, toggle, askNow } = useLiveCopilot();

  if (!isRecording) return null;

  // Collapsed: a single toggle chip.
  if (!enabled) {
    return (
      <div className="fixed bottom-24 right-6 z-40">
        <Button
          onClick={toggle}
          className="bg-blue-600 hover:bg-blue-700 text-white shadow-lg rounded-full"
          size="sm"
        >
          <Sparkles className="mr-2 h-4 w-4" />
          AI Copilot
        </Button>
      </div>
    );
  }

  return (
    <div className="fixed bottom-24 right-6 z-40 w-[360px] max-w-[90vw] rounded-xl border border-gray-200 bg-white shadow-2xl">
      <div className="flex items-center justify-between border-b px-4 py-2">
        <div className="flex items-center gap-2 text-sm font-semibold text-blue-700">
          <Sparkles className="h-4 w-4" />
          AI Copilot
          {isLoading && <RefreshCw className="h-3 w-3 animate-spin text-muted-foreground" />}
        </div>
        <div className="flex items-center gap-1">
          <Button variant="ghost" size="sm" onClick={askNow} disabled={isLoading} title="Ask now">
            <RefreshCw className="h-4 w-4" />
          </Button>
          <Button variant="ghost" size="sm" onClick={toggle} title="Hide copilot">
            <X className="h-4 w-4" />
          </Button>
        </div>
      </div>

      <div className="max-h-64 overflow-y-auto px-4 py-3 text-sm leading-relaxed">
        {error ? (
          <p className="text-red-600">{error}</p>
        ) : answer ? (
          <p className="whitespace-pre-wrap text-gray-800">{answer}</p>
        ) : (
          <p className="text-muted-foreground">
            Listening… I&apos;ll suggest an answer when a question comes up.
          </p>
        )}
      </div>
    </div>
  );
}
