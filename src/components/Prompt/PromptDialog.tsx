import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { useBackendEvent } from "@/hooks/useBackendEvent";
import { BackendClient } from "@/utils/backend/client";
import { BackendListener } from "@/utils/backend/listener";
import { PromptKind, PromptRequest } from "@/utils/backend/models";
import {
    Alert,
    Box,
    Button,
    Dialog,
    DialogActions,
    DialogContent,
    DialogTitle,
    TextField,
    Typography,
} from "@mui/material";
import { FormEvent, useContext, useState } from "react";

// Above the loading backdrop (1301), as prompts arrive while a task is running
const PROMPT_Z_INDEX = 1400;

type Field = { key: string; type: "text" | "email" | "password" };

const FIELDS: Record<PromptKind, Field[]> = {
    [PromptKind.ConnectCredentials]: [
        { key: "email", type: "email" },
        { key: "password", type: "password" },
    ],
    [PromptKind.ConnectMfa]: [{ key: "prompt_connect_mfa", type: "text" }],
    [PromptKind.SudoPassword]: [{ key: "password", type: "password" }],
};

const TITLES: Record<PromptKind, string> = {
    [PromptKind.ConnectCredentials]: "prompt_connect_credentials",
    [PromptKind.ConnectMfa]: "prompt_connect_mfa",
    [PromptKind.SudoPassword]: "prompt_sudo_password",
};

// Shows, one at a time, the inputs the backend asks for and sends back the answers
export function PromptDialog() {
    const [queue, setQueue] = useState<PromptRequest[]>([]);

    useBackendEvent(BackendListener.onPromptRequested, (request) => {
        setQueue((previous) => [...previous, request]);
    });

    const current = queue.at(0);
    if (!current) {
        return null;
    }

    const answer = (values: string[] | null) => {
        setQueue((previous) => previous.filter((r) => r.id !== current.id));
        BackendClient.answerPrompt(current.id, values).catch((err) =>
            console.error("Could not answer prompt " + current.id, err),
        );
    };

    // Keyed so every request starts with its own fresh fields
    return <PromptForm key={current.id} request={current} onAnswer={answer} />;
}

type FormProps = {
    request: PromptRequest;
    onAnswer: (values: string[] | null) => void;
};

function PromptForm({ request, onAnswer }: FormProps) {
    const { translate } = useContext(I18nSettingsContext);
    const fields = FIELDS[request.kind];

    const [values, setValues] = useState<string[]>(() =>
        fields.map((_, i) => (i == 0 ? (request.prefill ?? "") : "")),
    );

    const complete = values.every((v) => v.length > 0);

    const submit = (e: FormEvent) => {
        e.preventDefault();
        if (complete) {
            onAnswer(values);
        }
    };

    const message = (() => {
        switch (request.kind) {
            case PromptKind.ConnectMfa:
                return translate("prompt_connect_mfa_msg", [
                    request.detail ?? "",
                ]);
            case PromptKind.SudoPassword:
                return translate("prompt_sudo_password_msg");
            default:
                return null;
        }
    })();

    const error = (() => {
        if (!request.detail) {
            return null;
        }
        switch (request.kind) {
            case PromptKind.ConnectCredentials:
                return translate("prompt_connect_login_failed", [
                    request.detail,
                ]);
            case PromptKind.SudoPassword:
                return translate("prompt_sudo_wrong_password");
            default:
                return null;
        }
    })();

    // The first empty field gets the focus, so a prefilled email jumps straight to the password
    const focused = Math.max(
        0,
        values.findIndex((v) => v.length == 0),
    );

    return (
        <Dialog
            open={true}
            onClose={() => onAnswer(null)}
            sx={{ zIndex: PROMPT_Z_INDEX }}
            fullWidth
            maxWidth="xs"
        >
            <form onSubmit={submit}>
                <DialogTitle>{translate(TITLES[request.kind])}</DialogTitle>
                <DialogContent dividers>
                    <Box
                        sx={{
                            display: "flex",
                            flexDirection: "column",
                            gap: 2,
                        }}
                    >
                        {message && <Typography>{message}</Typography>}
                        {error && <Alert severity="error">{error}</Alert>}
                        {fields.map((field, i) => (
                            <TextField
                                key={field.key}
                                label={translate(field.key)}
                                type={field.type}
                                value={values[i]}
                                autoFocus={i == focused}
                                autoComplete="off"
                                size="small"
                                fullWidth
                                onChange={(e) =>
                                    setValues((previous) =>
                                        previous.map((v, j) =>
                                            j == i ? e.target.value : v,
                                        ),
                                    )
                                }
                            />
                        ))}
                    </Box>
                </DialogContent>
                <DialogActions>
                    <Button onClick={() => onAnswer(null)}>
                        {translate("cancel")}
                    </Button>
                    <Button
                        type="submit"
                        variant="contained"
                        disabled={!complete}
                    >
                        {translate("accept")}
                    </Button>
                </DialogActions>
            </form>
        </Dialog>
    );
}
