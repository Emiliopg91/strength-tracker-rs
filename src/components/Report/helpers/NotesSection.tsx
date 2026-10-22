import { JSX, useContext } from "react";
import PageContent from "./PageContent";
import { I18nSettingsContext } from "@/context/I18nSettingsContext";

export default function NotesSection(): JSX.Element {
    const { translate } = useContext(I18nSettingsContext);
    return (
        <PageContent>
            <h2>{translate("notes_section")}</h2>
            <div className="section-container">
                <h4>{translate("unilateral_note_title")}</h4>
                <div className="section-container">
                    {translate("unilateral_note_body")}
                </div>
            </div>
        </PageContent>
    );
}
