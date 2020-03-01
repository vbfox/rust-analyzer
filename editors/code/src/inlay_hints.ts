import * as vscode from 'vscode';
import * as ra from './rust-analyzer-api';

import { Ctx } from './ctx';
import { log, sendRequestWithRetry, arrayShallowEqual } from './util';
import { InlayHintsType } from './config';

export function activateInlayHints(ctx: Ctx) {
    const hintsUpdater = new HintsUpdater(ctx);
    vscode.window.onDidChangeVisibleTextEditors(
        async _ => hintsUpdater.refresh(),
        null,
        ctx.subscriptions
    );

    vscode.workspace.onDidChangeTextDocument(
        async event => {
            if (event.contentChanges.length === 0) return;
            if (event.document.languageId !== 'rust') return;
            await hintsUpdater.refresh();
        },
        null,
        ctx.subscriptions
    );

    vscode.workspace.onDidChangeConfiguration(
        async _ => hintsUpdater.setEnabled(ctx.config.displayInlayHints, ctx.config.inlayHintsTypes),
        null,
        ctx.subscriptions
    );

    ctx.pushCleanup({
        dispose() {
            hintsUpdater.clear();
        }
    });

    // XXX: we don't await this, thus Promise rejections won't be handled, but
    // this should never throw in fact...
    void hintsUpdater.setEnabled(ctx.config.displayInlayHints, ctx.config.inlayHintsTypes);
}

const typeHintDecorationType = vscode.window.createTextEditorDecorationType({
    after: {
        color: new vscode.ThemeColor('rust_analyzer.inlayHint'),
        fontStyle: "normal",
    },
});

const parameterHintDecorationType = vscode.window.createTextEditorDecorationType({
    before: {
        color: new vscode.ThemeColor('rust_analyzer.inlayHint'),
        fontStyle: "normal",
    },
});

class HintsUpdater {
    private pending = new Map<string, vscode.CancellationTokenSource>();
    private ctx: Ctx;
    private enabled: boolean;
    private inlayHintTypes: readonly InlayHintsType[];
    private showParameterName: boolean;
    private showVariableType: boolean;

    constructor(ctx: Ctx) {
        this.ctx = ctx;
        this.enabled = false;
        this.inlayHintTypes = [];
        this.showParameterName = false;
        this.showVariableType = false;

    }

    async setEnabled(enabled: boolean, inlayHintTypes: readonly InlayHintsType[]): Promise<void> {
        log.debug({ enabled, prev: this.enabled });

        if (this.enabled === enabled && arrayShallowEqual(inlayHintTypes, this.inlayHintTypes)) {
            return;
        }
        this.enabled = enabled;
        this.inlayHintTypes = inlayHintTypes;
        this.showParameterName = inlayHintTypes.some(t => t === InlayHintsType.ParameterName);
        this.showVariableType = inlayHintTypes.some(t => t === InlayHintsType.VariableType);

        if (this.enabled) {
            return await this.refresh();
        } else {
            return this.clear();
        }
    }

    clear() {
        this.allEditors.forEach(it => {
            this.setTypeDecorations(it, []);
            this.setParameterDecorations(it, []);
        });
    }

    async refresh() {
        if (!this.enabled) return;
        await Promise.all(this.allEditors.map(it => this.refreshEditor(it)));
    }

    private get allEditors(): vscode.TextEditor[] {
        return vscode.window.visibleTextEditors.filter(
            editor => editor.document.languageId === 'rust',
        );
    }

    private async refreshEditor(editor: vscode.TextEditor): Promise<void> {
        const newHints = await this.queryHints(editor.document.uri.toString());
        if (newHints == null) return;

        if (this.showVariableType) {
            const newTypeDecorations = newHints
                .filter(hint => hint.kind === ra.InlayKind.TypeHint)
                .map(hint => ({
                    range: this.ctx.client.protocol2CodeConverter.asRange(hint.range),
                    renderOptions: {
                        after: {
                            contentText: `: ${hint.label}`,
                        },
                    },
                }));
            this.setTypeDecorations(editor, newTypeDecorations);
        } else {
            this.setTypeDecorations(editor, []);
        }

        if (this.showParameterName) {
            const newParameterDecorations = newHints
                .filter(hint => hint.kind === ra.InlayKind.ParameterHint)
                .map(hint => ({
                    range: this.ctx.client.protocol2CodeConverter.asRange(hint.range),
                    renderOptions: {
                        before: {
                            contentText: `${hint.label}: `,
                        },
                    },
                }));
            this.setParameterDecorations(editor, newParameterDecorations);
        } else {
            this.setParameterDecorations(editor, []);
        }
    }

    private setTypeDecorations(
        editor: vscode.TextEditor,
        decorations: vscode.DecorationOptions[],
    ) {
        editor.setDecorations(
            typeHintDecorationType,
            this.enabled ? decorations : [],
        );
    }

    private setParameterDecorations(
        editor: vscode.TextEditor,
        decorations: vscode.DecorationOptions[],
    ) {
        editor.setDecorations(
            parameterHintDecorationType,
            this.enabled ? decorations : [],
        );
    }

    private async queryHints(documentUri: string): Promise<ra.InlayHint[] | null> {
        this.pending.get(documentUri)?.cancel();

        const tokenSource = new vscode.CancellationTokenSource();
        this.pending.set(documentUri, tokenSource);

        const request = { textDocument: { uri: documentUri } };

        return sendRequestWithRetry(this.ctx.client, ra.inlayHints, request, tokenSource.token)
            .catch(_ => null)
            .finally(() => {
                if (!tokenSource.token.isCancellationRequested) {
                    this.pending.delete(documentUri);
                }
            });
    }
}
