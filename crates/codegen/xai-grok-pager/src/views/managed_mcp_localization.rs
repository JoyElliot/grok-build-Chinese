//! Display-only localization identities for the official managed MCP gateway.
//!
//! Routing, search, permissions, schemas, arguments, and copy payloads continue
//! to use the canonical connector and qualified tool IDs. A translated
//! description is only selected when its complete current English source
//! matches the pinned SHA-256 digest.

use crate::locale::LocaleContext;
use sha2::{Digest, Sha256};
use xai_grok_tools::types::resources::ManagedGatewayToolIdentity;

#[derive(Debug, Clone, Copy)]
pub(crate) struct KnownManagedMcpTool {
    pub(crate) connector: &'static str,
    pub(crate) tool_id: &'static str,
    pub(crate) qualified_name: &'static str,
    pub(crate) display_name: &'static str,
    pub(crate) alias_english: &'static str,
    description_sha256: &'static str,
}

pub(crate) const KNOWN_MANAGED_MCP_TOOLS: &[KnownManagedMcpTool] = &[
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "actions_get",
        qualified_name: "github__actions_get",
        display_name: "Actions Get",
        alias_english: "GitHub Actions Get",
        description_sha256: "8f30e95f5bd88f80d07321083fc55ed572cb8f52f879946e12f375ed7670d58a",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "actions_list",
        qualified_name: "github__actions_list",
        display_name: "Actions List",
        alias_english: "GitHub Actions List",
        description_sha256: "ee3cc552f9dbdd567c4e4187f7f7d726847218593caca5c44fffc54f3be95330",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "actions_run_trigger",
        qualified_name: "github__actions_run_trigger",
        display_name: "Actions Run Trigger",
        alias_english: "GitHub Actions Run Trigger",
        description_sha256: "a505829777123c7abbf251131eb96c6bf08968c183761b3bf2f4664575e9134f",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "add_comment_to_pending_review",
        qualified_name: "github__add_comment_to_pending_review",
        display_name: "Add Comment To Pending Review",
        alias_english: "GitHub Add Comment To Pending Review",
        description_sha256: "c01d9b539761ed349fdefe576e822b51e45119bb65ec58bae048f32d545a4a59",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "add_issue_comment",
        qualified_name: "github__add_issue_comment",
        display_name: "Add Issue Comment",
        alias_english: "GitHub Add Issue Comment",
        description_sha256: "0eab077d2b8286c1e7dd2b70fa715cbed19b5bd4b8606dc369e3fdc04043d823",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "add_reply_to_pull_request_comment",
        qualified_name: "github__add_reply_to_pull_request_comment",
        display_name: "Add Reply To Pull Request Comment",
        alias_english: "GitHub Add Reply To Pull Request Comment",
        description_sha256: "bc6c858f67a973e0c6e87c7aab76ee31e81812f0d13e76ea1dc71a8e29c4a802",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "check_dependency_vulnerabilities",
        qualified_name: "github__check_dependency_vulnerabilities",
        display_name: "Check Dependency Vulnerabilities",
        alias_english: "GitHub Check Dependency Vulnerabilities",
        description_sha256: "caa7daa45dff837151c1839b5bca43ac9a8fafc292678761b08eeaae670cb619",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "create_branch",
        qualified_name: "github__create_branch",
        display_name: "Create Branch",
        alias_english: "GitHub Create Branch",
        description_sha256: "178c4aa2cad9c4dec2d6883eb0913ba5385f367e681e9d97cb751a2eb0983645",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "create_gist",
        qualified_name: "github__create_gist",
        display_name: "Create Gist",
        alias_english: "GitHub Create Gist",
        description_sha256: "470ab92c5dfc2582e8f9050a3804d5af2dde70c53d93e575e8c58136012e6c27",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "create_or_update_file",
        qualified_name: "github__create_or_update_file",
        display_name: "Create Or Update File",
        alias_english: "GitHub Create Or Update File",
        description_sha256: "493dc7efed597b08144d649a4699e306d2947048d1fa744c81ec824d8ffecacc",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "create_pull_request",
        qualified_name: "github__create_pull_request",
        display_name: "Create Pull Request",
        alias_english: "GitHub Create Pull Request",
        description_sha256: "b3ce1a8e1c8396e567b2df7957109ec2298ca873d8084f9a9c033f39657f3572",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "create_repository",
        qualified_name: "github__create_repository",
        display_name: "Create Repository",
        alias_english: "GitHub Create Repository",
        description_sha256: "4b58d95b681b9e48375400e581666ae89d51cbad25412a2f5de964da9ce8bf80",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "create_repository_ruleset",
        qualified_name: "github__create_repository_ruleset",
        display_name: "Create Repository Ruleset",
        alias_english: "GitHub Create Repository Ruleset",
        description_sha256: "beac2184e0df192ded487d8aa470ca4d819ec49b8a54e68bacae32ee63eab795",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "custom_properties_read",
        qualified_name: "github__custom_properties_read",
        display_name: "Custom Properties Read",
        alias_english: "GitHub Custom Properties Read",
        description_sha256: "cb8645aaa094d7049ad9ebb623d789b9900ee1e1a74effd9363427a6e4b7633c",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "custom_properties_write",
        qualified_name: "github__custom_properties_write",
        display_name: "Custom Properties Write",
        alias_english: "GitHub Custom Properties Write",
        description_sha256: "9a7b6c66ebfa42f7a113182f32da8c9edcb3ef8b5ed4c3214ee42db8438dd7fd",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "delete_file",
        qualified_name: "github__delete_file",
        display_name: "Delete File",
        alias_english: "GitHub Delete File",
        description_sha256: "a6706184f6656f1e0a1d8b6322d2c1c18bb3672a97cd2ac5bf71b0daf99e8900",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "discussion_comment_write",
        qualified_name: "github__discussion_comment_write",
        display_name: "Discussion Comment Write",
        alias_english: "GitHub Discussion Comment Write",
        description_sha256: "3fd77ef6033a95215ab02b7029f369890d18ac1480acd610937c8c217229e38c",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "dismiss_notification",
        qualified_name: "github__dismiss_notification",
        display_name: "Dismiss Notification",
        alias_english: "GitHub Dismiss Notification",
        description_sha256: "72806460489c61ba45e9f10a43ff5b5f5cf5d43155b64b4d192cffe3979c0305",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "fork_repository",
        qualified_name: "github__fork_repository",
        display_name: "Fork Repository",
        alias_english: "GitHub Fork Repository",
        description_sha256: "b9c81712c56e48175df559052b73f7e28646208f961b6b61c3ac3f3545eef86f",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "get_code_quality_finding",
        qualified_name: "github__get_code_quality_finding",
        display_name: "Get Code Quality Finding",
        alias_english: "GitHub Get Code Quality Finding",
        description_sha256: "a2ea34687563cde90b4890df042e53cca9e024912d997d53a9c52824bd645877",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "get_code_scanning_alert",
        qualified_name: "github__get_code_scanning_alert",
        display_name: "Get Code Scanning Alert",
        alias_english: "GitHub Get Code Scanning Alert",
        description_sha256: "c9355e6046bba99a24d2d56a7b7ae04bd213029c8921890e6a080b11cf924a17",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "get_commit",
        qualified_name: "github__get_commit",
        display_name: "Get Commit",
        alias_english: "GitHub Get Commit",
        description_sha256: "a27095bf05dc570a18bf4f6db26662c8dd39f2997f914127c59e8ecf906bf30f",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "get_copilot_space",
        qualified_name: "github__get_copilot_space",
        display_name: "Get Copilot Space",
        alias_english: "GitHub Get Copilot Space",
        description_sha256: "9e60bce9aa9e04adb127fcd844fae2beea405cbd765f1f056979453369f03ee8",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "get_dependabot_alert",
        qualified_name: "github__get_dependabot_alert",
        display_name: "Get Dependabot Alert",
        alias_english: "GitHub Get Dependabot Alert",
        description_sha256: "de61bf255daddafdd68fb620fa2abc05f2e527c944d8bb4d1424d9e72c06663e",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "get_discussion",
        qualified_name: "github__get_discussion",
        display_name: "Get Discussion",
        alias_english: "GitHub Get Discussion",
        description_sha256: "e87e063be57c64bc4e7f333a4e4428d2552f1f09e32045d2b9e89845e8762b82",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "get_discussion_comments",
        qualified_name: "github__get_discussion_comments",
        display_name: "Get Discussion Comments",
        alias_english: "GitHub Get Discussion Comments",
        description_sha256: "e4b8f713aea3d38d19578af518ccbf7c40a75b11d5391e19d4356abe600eb1bd",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "get_file_contents",
        qualified_name: "github__get_file_contents",
        display_name: "Get File Contents",
        alias_english: "GitHub Get File Contents",
        description_sha256: "54de6216aa12cd8da08e335b6955e2261b4241359f184959829407d0e40dcdc0",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "get_gist",
        qualified_name: "github__get_gist",
        display_name: "Get Gist",
        alias_english: "GitHub Get Gist",
        description_sha256: "b13ead29e27dd15ba2168a45cd5935562d0d282cffc79daad6eab8fefeb8232b",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "get_global_security_advisory",
        qualified_name: "github__get_global_security_advisory",
        display_name: "Get Global Security Advisory",
        alias_english: "GitHub Get Global Security Advisory",
        description_sha256: "8e3dc5a5359eee0a5fc561b61ed051d3619d5c4e27aaad598d664bcbd0011901",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "get_job_logs",
        qualified_name: "github__get_job_logs",
        display_name: "Get Job Logs",
        alias_english: "GitHub Get Job Logs",
        description_sha256: "4effcfb9a2d3b9336cfe75149f127cc2eba0fde51b19832212aa81a256e6e732",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "get_label",
        qualified_name: "github__get_label",
        display_name: "Get Label",
        alias_english: "GitHub Get Label",
        description_sha256: "bc5e986298d736683f2928e24dd080fa0735fbcb3d1529aa2573a84570568b44",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "get_latest_release",
        qualified_name: "github__get_latest_release",
        display_name: "Get Latest Release",
        alias_english: "GitHub Get Latest Release",
        description_sha256: "57a49eb576b15e088997f3906897973907a872ac7532593fa48826e0b3d0d09a",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "get_me",
        qualified_name: "github__get_me",
        display_name: "Get Me",
        alias_english: "GitHub Get Me",
        description_sha256: "bc34f566cc782d563dbfb6035ec4b94c7c7d46f34ef84c61cd7b02729ba281ce",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "get_notification_details",
        qualified_name: "github__get_notification_details",
        display_name: "Get Notification Details",
        alias_english: "GitHub Get Notification Details",
        description_sha256: "ec76845152fc49b3d76ac0087fe8752555ea3631b04d04d6a8d0f153cb0e1176",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "get_release_by_tag",
        qualified_name: "github__get_release_by_tag",
        display_name: "Get Release By Tag",
        alias_english: "GitHub Get Release By Tag",
        description_sha256: "370170693dc5177b119f9aadd27bb305c23eec6de6050e5accbee27acd764a7f",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "get_repository_tree",
        qualified_name: "github__get_repository_tree",
        display_name: "Get Repository Tree",
        alias_english: "GitHub Get Repository Tree",
        description_sha256: "4355bf11fa971efb885ebbb66191d5cbdf0f206c2bae54a49b0b2243933b6240",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "get_secret_scanning_alert",
        qualified_name: "github__get_secret_scanning_alert",
        display_name: "Get Secret Scanning Alert",
        alias_english: "GitHub Get Secret Scanning Alert",
        description_sha256: "0cc5a272aafe264f496df0317c38e5b24c554afbc136cfe98919d2447663e5c3",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "get_tag",
        qualified_name: "github__get_tag",
        display_name: "Get Tag",
        alias_english: "GitHub Get Tag",
        description_sha256: "e6d557e07eb01ac88760ac5a62bc68d3b795b61d4d7fa4be36758c0f7ce61eae",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "get_team_members",
        qualified_name: "github__get_team_members",
        display_name: "Get Team Members",
        alias_english: "GitHub Get Team Members",
        description_sha256: "e86ce60eeea8d7fcc9a5e50ae24c13b083aed10d254af864402ec8167502bbc4",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "get_teams",
        qualified_name: "github__get_teams",
        display_name: "Get Teams",
        alias_english: "GitHub Get Teams",
        description_sha256: "99380d708092b4760246658a3e9bc5f7991d7bcecc75c3dea03e13fcff6aa27b",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "github_support_docs_search",
        qualified_name: "github__github_support_docs_search",
        display_name: "Github Support Docs Search",
        alias_english: "GitHub Github Support Docs Search",
        description_sha256: "b1f62ff0baa8fbf1953d442fa443cecdacfb089c5e6692ab6724ec1487da75f2",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "issue_read",
        qualified_name: "github__issue_read",
        display_name: "Issue Read",
        alias_english: "GitHub Issue Read",
        description_sha256: "e3ccc7984b309935391ec33b448056c6177cd97383005727831d46e8c73213dc",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "issue_write",
        qualified_name: "github__issue_write",
        display_name: "Issue Write",
        alias_english: "GitHub Issue Write",
        description_sha256: "8fe78ac80b6a1295e9149d41aab56393236e2c6155abf09a5b56a23b09ef587c",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "label_write",
        qualified_name: "github__label_write",
        display_name: "Label Write",
        alias_english: "GitHub Label Write",
        description_sha256: "396dfa0cc748c53ff193b0f49d60461201a45620c071ed5493baa989d9ed6ba3",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "list_branches",
        qualified_name: "github__list_branches",
        display_name: "List Branches",
        alias_english: "GitHub List Branches",
        description_sha256: "8ce903bf8c1572fd527fd93f38d7d2ccb9b8d463ffe947100aeb1b8187363840",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "list_code_scanning_alerts",
        qualified_name: "github__list_code_scanning_alerts",
        display_name: "List Code Scanning Alerts",
        alias_english: "GitHub List Code Scanning Alerts",
        description_sha256: "2157c013472c46218c4a0315e1b0ba5e6eb9315cf7065b1f572d0a4c25fd7db7",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "list_commits",
        qualified_name: "github__list_commits",
        display_name: "List Commits",
        alias_english: "GitHub List Commits",
        description_sha256: "dd2e7a438ec8ef9f8c31a41ce203325fc971ad1dc601c7647f5a9a39ca372df9",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "list_copilot_spaces",
        qualified_name: "github__list_copilot_spaces",
        display_name: "List Copilot Spaces",
        alias_english: "GitHub List Copilot Spaces",
        description_sha256: "0929cf92a74637758a676a2b28b9c0979dde04e5a1fa02472e015722d4a5729a",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "list_dependabot_alerts",
        qualified_name: "github__list_dependabot_alerts",
        display_name: "List Dependabot Alerts",
        alias_english: "GitHub List Dependabot Alerts",
        description_sha256: "f3d1993f579cbecb1f6bd936f4ef979795480076df67269b29b88b1d511afdb4",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "list_discussion_categories",
        qualified_name: "github__list_discussion_categories",
        display_name: "List Discussion Categories",
        alias_english: "GitHub List Discussion Categories",
        description_sha256: "47fdb72ed8824c792c722f9ca7aea07cfab4a7b42b033335ccf5dbe0a99fb36a",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "list_discussions",
        qualified_name: "github__list_discussions",
        display_name: "List Discussions",
        alias_english: "GitHub List Discussions",
        description_sha256: "aa64efa50c1e8214b205eafc506af2baa44dd0db3c5b736ac5e61c59cf30b17a",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "list_gists",
        qualified_name: "github__list_gists",
        display_name: "List Gists",
        alias_english: "GitHub List Gists",
        description_sha256: "edc457069633633897b6a9beb60019ba520577da7e17e35009f03bca55414b89",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "list_global_security_advisories",
        qualified_name: "github__list_global_security_advisories",
        display_name: "List Global Security Advisories",
        alias_english: "GitHub List Global Security Advisories",
        description_sha256: "ee29194c45463bf282d5e9fc3185e3839609a22232c82d26183b560155ac1895",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "list_issue_fields",
        qualified_name: "github__list_issue_fields",
        display_name: "List Issue Fields",
        alias_english: "GitHub List Issue Fields",
        description_sha256: "2dd58d0918a55cb41fb3a01020cd25e1a70552019539c3c9fda032cb5bf84148",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "list_issue_types",
        qualified_name: "github__list_issue_types",
        display_name: "List Issue Types",
        alias_english: "GitHub List Issue Types",
        description_sha256: "044c396ff868d9a6abaf2b91817c1945b557c2e41556424076be9085f444eefc",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "list_issues",
        qualified_name: "github__list_issues",
        display_name: "List Issues",
        alias_english: "GitHub List Issues",
        description_sha256: "c41469eaf78f99580e51ff1bbbadc2922bdec37e47f0e5d142e1e576f3390c87",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "list_label",
        qualified_name: "github__list_label",
        display_name: "List Label",
        alias_english: "GitHub List Label",
        description_sha256: "7aabe8ae1af6bfeef25ce162b884008b891df44fef9dbeefc4560551814f24ee",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "list_notifications",
        qualified_name: "github__list_notifications",
        display_name: "List Notifications",
        alias_english: "GitHub List Notifications",
        description_sha256: "d10e656b1bf56afd6198d99dfbacab9b89240e71050cb766e6fc4e1952e4cc1c",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "list_org_repository_security_advisories",
        qualified_name: "github__list_org_repository_security_advisories",
        display_name: "List Org Repository Security Advisories",
        alias_english: "GitHub List Org Repository Security Advisories",
        description_sha256: "b955d6ac99e964a73854c319abfd5a5ec2efb69033a2ed20bf06da82b70a6c3b",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "list_pull_requests",
        qualified_name: "github__list_pull_requests",
        display_name: "List Pull Requests",
        alias_english: "GitHub List Pull Requests",
        description_sha256: "c249adc3491b598845fda74d1b7f815b368107b47786634fc6e44ef0ea5f1a06",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "list_releases",
        qualified_name: "github__list_releases",
        display_name: "List Releases",
        alias_english: "GitHub List Releases",
        description_sha256: "16c40a2d80141352b60b845be6bb163ab868e1dc3b7edbdbe14ca7b2d664e411",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "list_repository_collaborators",
        qualified_name: "github__list_repository_collaborators",
        display_name: "List Repository Collaborators",
        alias_english: "GitHub List Repository Collaborators",
        description_sha256: "adbe2ab4be9b09cf09ea46cfb15b658ce43ff32bb16e327d3081b5ea51ff9302",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "list_repository_security_advisories",
        qualified_name: "github__list_repository_security_advisories",
        display_name: "List Repository Security Advisories",
        alias_english: "GitHub List Repository Security Advisories",
        description_sha256: "d7b752d7baa2ddc100118971ce5e4de114e09cf1ec628ac3c41fb0e531529310",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "list_secret_scanning_alerts",
        qualified_name: "github__list_secret_scanning_alerts",
        display_name: "List Secret Scanning Alerts",
        alias_english: "GitHub List Secret Scanning Alerts",
        description_sha256: "3894671d369d1afd5626bc7a85fd304dc23c40e42ac99eab42ef7472f50cf231",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "list_starred_repositories",
        qualified_name: "github__list_starred_repositories",
        display_name: "List Starred Repositories",
        alias_english: "GitHub List Starred Repositories",
        description_sha256: "d852730315dcbc1456dc6bd0c56f02668779ce0e02cf3be75ee9b49b8edda8ca",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "list_tags",
        qualified_name: "github__list_tags",
        display_name: "List Tags",
        alias_english: "GitHub List Tags",
        description_sha256: "b45b57651e9a56b5d03befc9edb790d1c1d92742cc6e1cd9d56f6b41fc3dca92",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "manage_notification_subscription",
        qualified_name: "github__manage_notification_subscription",
        display_name: "Manage Notification Subscription",
        alias_english: "GitHub Manage Notification Subscription",
        description_sha256: "be32b04a7ce2d90c4cf1ba0bfe8674b5eeed86fdac39522df6afe856d12b0a06",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "manage_repository_notification_subscription",
        qualified_name: "github__manage_repository_notification_subscription",
        display_name: "Manage Repository Notification Subscription",
        alias_english: "GitHub Manage Repository Notification Subscription",
        description_sha256: "97e8f7279d5f6b8b031b73e5ecc55d093571b3eb9d4e244a65bc9acd31e907a1",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "mark_all_notifications_read",
        qualified_name: "github__mark_all_notifications_read",
        display_name: "Mark All Notifications Read",
        alias_english: "GitHub Mark All Notifications Read",
        description_sha256: "87e6c2a922e258ce8d6383d847f1cb480037d95d9baa6d366cf10fbca63a4c0b",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "merge_pull_request",
        qualified_name: "github__merge_pull_request",
        display_name: "Merge Pull Request",
        alias_english: "GitHub Merge Pull Request",
        description_sha256: "124cd641ce348386107609b1831084962d2198fa82fe58f7a040dd7e1cebb6b4",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "projects_get",
        qualified_name: "github__projects_get",
        display_name: "Projects Get",
        alias_english: "GitHub Projects Get",
        description_sha256: "6cdcd09c32ebb14ba7f1c379599cd2032893706a61d96891cc542edc7b2e32b9",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "projects_list",
        qualified_name: "github__projects_list",
        display_name: "Projects List",
        alias_english: "GitHub Projects List",
        description_sha256: "67993147da2048576a2e40ccbf2fa1335123e7b8d9b9bcf579addd2df0d6866c",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "projects_write",
        qualified_name: "github__projects_write",
        display_name: "Projects Write",
        alias_english: "GitHub Projects Write",
        description_sha256: "fda4533debc6e51d63ab3b9d573f35a8be2891d56a151a5ff6cc983132818869",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "pull_request_read",
        qualified_name: "github__pull_request_read",
        display_name: "Pull Request Read",
        alias_english: "GitHub Pull Request Read",
        description_sha256: "2d2b3f1fbb088bc1a5eef4fd77b7c8abdfd2753e2356abce8401dba5236cae5b",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "pull_request_review_write",
        qualified_name: "github__pull_request_review_write",
        display_name: "Pull Request Review Write",
        alias_english: "GitHub Pull Request Review Write",
        description_sha256: "6f34129d431b032f859853d8dcf48a9523a5ad6fa66079f2126c625f97f38505",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "push_files",
        qualified_name: "github__push_files",
        display_name: "Push Files",
        alias_english: "GitHub Push Files",
        description_sha256: "0ea99ad23e44e739ed503658bdaab5ee2dc239246cb00e715d8fff3d80fe544f",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "repository_ruleset_read",
        qualified_name: "github__repository_ruleset_read",
        display_name: "Repository Ruleset Read",
        alias_english: "GitHub Repository Ruleset Read",
        description_sha256: "a7407bc1e7b60ea3e2202f0d80f1b79aa2afe94e998a101b373d65934697f8f0",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "request_copilot_review",
        qualified_name: "github__request_copilot_review",
        display_name: "Request Copilot Review",
        alias_english: "GitHub Request Copilot Review",
        description_sha256: "0a31c498daefdb4310ae1335e16496ed8d238d01ebf12c04d45a1b215e4c7de3",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "run_secret_scanning",
        qualified_name: "github__run_secret_scanning",
        display_name: "Run Secret Scanning",
        alias_english: "GitHub Run Secret Scanning",
        description_sha256: "dd3b32257b64f6b2b4a4c07056fd0dce39426c1ab5779b48eea8b52ad420e484",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "search_code",
        qualified_name: "github__search_code",
        display_name: "Search Code",
        alias_english: "GitHub Search Code",
        description_sha256: "80d70342e3a3eb8b9ad5df5eb159840c6a363b7ef54bc757e541990984e2b2ad",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "search_commits",
        qualified_name: "github__search_commits",
        display_name: "Search Commits",
        alias_english: "GitHub Search Commits",
        description_sha256: "2daaa82a55c91ba3ddcec7d0fb2c16da11e46647600ca111e00ad857ac49efe0",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "search_issues",
        qualified_name: "github__search_issues",
        display_name: "Search Issues",
        alias_english: "GitHub Search Issues",
        description_sha256: "54ee7223ca0f61d33f3f4aa67dc97e6f0c704a18fc3218000eb39abc92c42484",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "search_orgs",
        qualified_name: "github__search_orgs",
        display_name: "Search Orgs",
        alias_english: "GitHub Search Orgs",
        description_sha256: "47cc29324a4a0f10b4d80043fd9bfc38b2d6c197f73c07ef7febdc172e0daad0",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "search_pull_requests",
        qualified_name: "github__search_pull_requests",
        display_name: "Search Pull Requests",
        alias_english: "GitHub Search Pull Requests",
        description_sha256: "d8a220faae6baa7cd5dfad6e2acd46e0949ef79854cd1d98baa8c6a5e15b1cea",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "search_repositories",
        qualified_name: "github__search_repositories",
        display_name: "Search Repositories",
        alias_english: "GitHub Search Repositories",
        description_sha256: "7b9c5ffba195b04b1c4d835eca98ea84c999b254239740dd5a38e89d6f46ab02",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "search_users",
        qualified_name: "github__search_users",
        display_name: "Search Users",
        alias_english: "GitHub Search Users",
        description_sha256: "e2c14890a74e50c883b5ba65dd1ae521152ef8e7ffe67aab5336091fcefe0807",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "semantic_issue_similarity_search",
        qualified_name: "github__semantic_issue_similarity_search",
        display_name: "Semantic Issue Similarity Search",
        alias_english: "GitHub Semantic Issue Similarity Search",
        description_sha256: "24fcbcb6c3c2b968e0fc72a7376713c757757e2bab0d20e4c89827166536e87b",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "star_repository",
        qualified_name: "github__star_repository",
        display_name: "Star Repository",
        alias_english: "GitHub Star Repository",
        description_sha256: "79c19b991156ccefedaabb14aea64256d471ffb34449c90119670b9a2ddaf189",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "sub_issue_write",
        qualified_name: "github__sub_issue_write",
        display_name: "Sub Issue Write",
        alias_english: "GitHub Sub Issue Write",
        description_sha256: "0e8fa66b77f7ec60fa9ee3a6402d1aa53a8b4ee45621b52dea19a7777b9692c9",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "unstar_repository",
        qualified_name: "github__unstar_repository",
        display_name: "Unstar Repository",
        alias_english: "GitHub Unstar Repository",
        description_sha256: "2c008970d750298bb698d004709a456c8dd27322a7c84a456745cade3c07b49d",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "update_gist",
        qualified_name: "github__update_gist",
        display_name: "Update Gist",
        alias_english: "GitHub Update Gist",
        description_sha256: "2064a30fbd0677d3a7e2fd52841e10d1851ea33eae47dd08d1da8fa5ef529bc6",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "update_pull_request",
        qualified_name: "github__update_pull_request",
        display_name: "Update Pull Request",
        alias_english: "GitHub Update Pull Request",
        description_sha256: "bed4d74cfd86d23ab02749d6b4fffa5ba43c3290bfa7c9810514cf821e0563eb",
    },
    KnownManagedMcpTool {
        connector: "github",
        tool_id: "update_pull_request_branch",
        qualified_name: "github__update_pull_request_branch",
        display_name: "Update Pull Request Branch",
        alias_english: "GitHub Update Pull Request Branch",
        description_sha256: "bb1dacdad1b56b12c6b26f7833d5b189a7827f66ea3d04917632eed63277d80d",
    },
    KnownManagedMcpTool {
        connector: "gmail",
        tool_id: "batch_modify_labels",
        qualified_name: "gmail__batch_modify_labels",
        display_name: "Batch Modify Labels",
        alias_english: "Gmail Batch Modify Labels",
        description_sha256: "deab42f8cce1ebf8918d374d2891466b2bd5e9e3abbf5cd6f5d44a0f1617daeb",
    },
    KnownManagedMcpTool {
        connector: "gmail",
        tool_id: "create_draft",
        qualified_name: "gmail__create_draft",
        display_name: "Create Draft",
        alias_english: "Gmail Create Draft",
        description_sha256: "eb467d4924e0ccad49546b02bc0f769e6f084568c36b75850be0716a122ecbd0",
    },
    KnownManagedMcpTool {
        connector: "gmail",
        tool_id: "create_label",
        qualified_name: "gmail__create_label",
        display_name: "Create Label",
        alias_english: "Gmail Create Label",
        description_sha256: "14ec18e035c8faa1a11887e5ae9402c9909e3065a5726cc287ceefb718072644",
    },
    KnownManagedMcpTool {
        connector: "gmail",
        tool_id: "delete_draft",
        qualified_name: "gmail__delete_draft",
        display_name: "Delete Draft",
        alias_english: "Gmail Delete Draft",
        description_sha256: "12697521b1e5bee23dff1c75ccf375b4b5055a458c43abd9a448fd39e170112b",
    },
    KnownManagedMcpTool {
        connector: "gmail",
        tool_id: "delete_label",
        qualified_name: "gmail__delete_label",
        display_name: "Delete Label",
        alias_english: "Gmail Delete Label",
        description_sha256: "42ebe7129bd79604dfd57b338b6b21255aa2e6fcdfad93a06ea14e75bf676114",
    },
    KnownManagedMcpTool {
        connector: "gmail",
        tool_id: "forward_message",
        qualified_name: "gmail__forward_message",
        display_name: "Forward Message",
        alias_english: "Gmail Forward Message",
        description_sha256: "b594e0eda252bdfb80ca96e168439957e31c572f3941ed8a37ab576376858cbe",
    },
    KnownManagedMcpTool {
        connector: "gmail",
        tool_id: "get_message",
        qualified_name: "gmail__get_message",
        display_name: "Get Message",
        alias_english: "Gmail Get Message",
        description_sha256: "ec15cd86e777a346021919ff8b8c25b284f6f5b6950aea6d34223157e2495b30",
    },
    KnownManagedMcpTool {
        connector: "gmail",
        tool_id: "list_drafts",
        qualified_name: "gmail__list_drafts",
        display_name: "List Drafts",
        alias_english: "Gmail List Drafts",
        description_sha256: "b93dd3034323f83e569f4b5607794198a6b975abcd8631eaafd9f31a914fd271",
    },
    KnownManagedMcpTool {
        connector: "gmail",
        tool_id: "list_labels",
        qualified_name: "gmail__list_labels",
        display_name: "List Labels",
        alias_english: "Gmail List Labels",
        description_sha256: "01fcb91869424d4fd950a55d72279d1d255137b0545021bb5a6e77071135d096",
    },
    KnownManagedMcpTool {
        connector: "gmail",
        tool_id: "modify_labels",
        qualified_name: "gmail__modify_labels",
        display_name: "Modify Labels",
        alias_english: "Gmail Modify Labels",
        description_sha256: "71c544f2f850db00b93531dab6fa6968daed1213ce89ede58213981792d32a6e",
    },
    KnownManagedMcpTool {
        connector: "gmail",
        tool_id: "reply_all",
        qualified_name: "gmail__reply_all",
        display_name: "Reply All",
        alias_english: "Gmail Reply All",
        description_sha256: "21958d72f7e691c02f6b2286011e08d2b0f913240c5c075198e230d360aa4cb2",
    },
    KnownManagedMcpTool {
        connector: "gmail",
        tool_id: "search",
        qualified_name: "gmail__search",
        display_name: "Search",
        alias_english: "Gmail Search",
        description_sha256: "adb1d9b4b27e3c26ccfb9caf769038b1b77a50226dddfecad8f3f94e59421b47",
    },
    KnownManagedMcpTool {
        connector: "gmail",
        tool_id: "send_draft",
        qualified_name: "gmail__send_draft",
        display_name: "Send Draft",
        alias_english: "Gmail Send Draft",
        description_sha256: "01f512fa33af1d072fd2fd5b746d90998c4029a83d11d59ca2d5d86178b46b82",
    },
    KnownManagedMcpTool {
        connector: "gmail",
        tool_id: "send_message",
        qualified_name: "gmail__send_message",
        display_name: "Send Message",
        alias_english: "Gmail Send Message",
        description_sha256: "c6e18eefda87bf2fac1abaac03a605e880775737e86fcd28094422f02d0c349b",
    },
    KnownManagedMcpTool {
        connector: "gmail",
        tool_id: "trash_message",
        qualified_name: "gmail__trash_message",
        display_name: "Trash Message",
        alias_english: "Gmail Trash Message",
        description_sha256: "19eab64eca2f46854252879aa55fbcabadab0ff2ac55387685e7fb01d68ac7d2",
    },
    KnownManagedMcpTool {
        connector: "gmail",
        tool_id: "update_draft",
        qualified_name: "gmail__update_draft",
        display_name: "Update Draft",
        alias_english: "Gmail Update Draft",
        description_sha256: "89d19110d95dabe443d8e5c842995721213f84874a2a9f4e52b85c6c32befab1",
    },
    KnownManagedMcpTool {
        connector: "outlook",
        tool_id: "batch_move",
        qualified_name: "outlook__batch_move",
        display_name: "Batch Move",
        alias_english: "Outlook Batch Move",
        description_sha256: "fdd0d52109d23bf5ca1598e70e9771c90e7fa12881a4b3f0ffa4bcc45c241501",
    },
    KnownManagedMcpTool {
        connector: "outlook",
        tool_id: "create_draft",
        qualified_name: "outlook__create_draft",
        display_name: "Create Draft",
        alias_english: "Outlook Create Draft",
        description_sha256: "f12fb9720604ba0d7cceaf009a01ed447bf4a0a107b8f687465ce933579e2ce1",
    },
    KnownManagedMcpTool {
        connector: "outlook",
        tool_id: "create_folder",
        qualified_name: "outlook__create_folder",
        display_name: "Create Folder",
        alias_english: "Outlook Create Folder",
        description_sha256: "bdb397b2e255e1153c3b594e52660f30e9db0ec33025f65e7afd533da9a950ee",
    },
    KnownManagedMcpTool {
        connector: "outlook",
        tool_id: "delete_draft",
        qualified_name: "outlook__delete_draft",
        display_name: "Delete Draft",
        alias_english: "Outlook Delete Draft",
        description_sha256: "befb6add66d78ba5061d623c3b968aaf7513a43d0a3e51057ce22e401dcd50e7",
    },
    KnownManagedMcpTool {
        connector: "outlook",
        tool_id: "delete_folder",
        qualified_name: "outlook__delete_folder",
        display_name: "Delete Folder",
        alias_english: "Outlook Delete Folder",
        description_sha256: "0d5a756ce979340a9b343278e0b1a9baecb61a2f0fe1e47f28a23433bda99f2b",
    },
    KnownManagedMcpTool {
        connector: "outlook",
        tool_id: "delete_message",
        qualified_name: "outlook__delete_message",
        display_name: "Delete Message",
        alias_english: "Outlook Delete Message",
        description_sha256: "6ae87d659ae62a2fa792ebca25acbbe0a0b268e4a35a1e9df4c612c18a1b1cc0",
    },
    KnownManagedMcpTool {
        connector: "outlook",
        tool_id: "forward_message",
        qualified_name: "outlook__forward_message",
        display_name: "Forward Message",
        alias_english: "Outlook Forward Message",
        description_sha256: "918ae265ecd92442dbb6bed42662983376fdd9909ef78e49228e5f2ba8231cf5",
    },
    KnownManagedMcpTool {
        connector: "outlook",
        tool_id: "get_message",
        qualified_name: "outlook__get_message",
        display_name: "Get Message",
        alias_english: "Outlook Get Message",
        description_sha256: "1931984ae1655e2c4f923674d40312117d9deaf23e2efa8e0c15c04f30e454ca",
    },
    KnownManagedMcpTool {
        connector: "outlook",
        tool_id: "list_drafts",
        qualified_name: "outlook__list_drafts",
        display_name: "List Drafts",
        alias_english: "Outlook List Drafts",
        description_sha256: "31e4daba84cce1b5ae6d209fda93332d24c64a0d4cb12a863fe5f1cc1efe8e09",
    },
    KnownManagedMcpTool {
        connector: "outlook",
        tool_id: "list_folders",
        qualified_name: "outlook__list_folders",
        display_name: "List Folders",
        alias_english: "Outlook List Folders",
        description_sha256: "f0badfc613e1063709c1747324041132554554bc646d019abf4a3ce03d12bfa0",
    },
    KnownManagedMcpTool {
        connector: "outlook",
        tool_id: "modify_message",
        qualified_name: "outlook__modify_message",
        display_name: "Modify Message",
        alias_english: "Outlook Modify Message",
        description_sha256: "162c2199208cb685c6140f0c79c14618830800d48b466c6415111c77b83b26a9",
    },
    KnownManagedMcpTool {
        connector: "outlook",
        tool_id: "move_message",
        qualified_name: "outlook__move_message",
        display_name: "Move Message",
        alias_english: "Outlook Move Message",
        description_sha256: "639b3f9fe55bc07db8add723a7211c49b4e8372b189b101a6c07e0f4b4831d0b",
    },
    KnownManagedMcpTool {
        connector: "outlook",
        tool_id: "reply_all",
        qualified_name: "outlook__reply_all",
        display_name: "Reply All",
        alias_english: "Outlook Reply All",
        description_sha256: "7540e65516711e38d14743e504547e633c12d5d68d9e69eb8bdf24a0ada5dd04",
    },
    KnownManagedMcpTool {
        connector: "outlook",
        tool_id: "search",
        qualified_name: "outlook__search",
        display_name: "Search",
        alias_english: "Outlook Search",
        description_sha256: "103c4a3c48dfd03ecddc68f5773cdfd800c52f5f6a231336eaa921dfc369d80d",
    },
    KnownManagedMcpTool {
        connector: "outlook",
        tool_id: "send_draft",
        qualified_name: "outlook__send_draft",
        display_name: "Send Draft",
        alias_english: "Outlook Send Draft",
        description_sha256: "e2be989633d23e7c8921ef8d1fd8289111368ba2e45dd72711daaf092c591451",
    },
    KnownManagedMcpTool {
        connector: "outlook",
        tool_id: "send_message",
        qualified_name: "outlook__send_message",
        display_name: "Send Message",
        alias_english: "Outlook Send Message",
        description_sha256: "a07df2dbb11e6469365e21bf194b7879712b98480bc352ef9362e97a0ddb51a5",
    },
    KnownManagedMcpTool {
        connector: "outlook",
        tool_id: "trash_message",
        qualified_name: "outlook__trash_message",
        display_name: "Trash Message",
        alias_english: "Outlook Trash Message",
        description_sha256: "f7a174dc8f871e7bcf9f774f13245f7fe593eb8dd65e9026754c7b4265219888",
    },
    KnownManagedMcpTool {
        connector: "outlook",
        tool_id: "update_draft",
        qualified_name: "outlook__update_draft",
        display_name: "Update Draft",
        alias_english: "Outlook Update Draft",
        description_sha256: "3f746819a5fda95bfe6b7d9e5f824f36a35de3874e1199d2682361cb8ba75185",
    },
    KnownManagedMcpTool {
        connector: "tasks",
        tool_id: "create",
        qualified_name: "tasks__create",
        display_name: "Create",
        alias_english: "Tasks Create",
        description_sha256: "bd70559a0f8696630e9bf97cb571d50a48cbb487f90fc1b75a9e6e32ebb65570",
    },
    KnownManagedMcpTool {
        connector: "tasks",
        tool_id: "delete",
        qualified_name: "tasks__delete",
        display_name: "Delete",
        alias_english: "Tasks Delete",
        description_sha256: "127ff8c35578884847a40616c03d5bdf3b44785b3927f40859f8cddbb82bcbf2",
    },
    KnownManagedMcpTool {
        connector: "tasks",
        tool_id: "get_results",
        qualified_name: "tasks__get_results",
        display_name: "Get Results",
        alias_english: "Tasks Get Results",
        description_sha256: "fcde7ab85e189428c7507a7cfbfc68e06a869f6c2e9841cd58a8315fce15dfa4",
    },
    KnownManagedMcpTool {
        connector: "tasks",
        tool_id: "list",
        qualified_name: "tasks__list",
        display_name: "List",
        alias_english: "Tasks List",
        description_sha256: "d92bdcbd0f8b0a9b2d010d43e72bf3f29b7044d929dcedac4822d91770a292fc",
    },
    KnownManagedMcpTool {
        connector: "tasks",
        tool_id: "list_trigger_catalog",
        qualified_name: "tasks__list_trigger_catalog",
        display_name: "List Trigger Catalog",
        alias_english: "Tasks List Trigger Catalog",
        description_sha256: "c5291881d5fba7ee86c831d90105a9e78d5394c7d1a10323d1b91a4bb3dd8a14",
    },
    KnownManagedMcpTool {
        connector: "tasks",
        tool_id: "list_trigger_resources",
        qualified_name: "tasks__list_trigger_resources",
        display_name: "List Trigger Resources",
        alias_english: "Tasks List Trigger Resources",
        description_sha256: "d5b460c32291fdd8a23c041c40aca4e7e2ba451cccd323940c129047b36276cc",
    },
    KnownManagedMcpTool {
        connector: "tasks",
        tool_id: "pause",
        qualified_name: "tasks__pause",
        display_name: "Pause",
        alias_english: "Tasks Pause",
        description_sha256: "24aa2383616a0ed4f3a8db305fbd5ccf2731f92dab024b038d4f70e920a92940",
    },
    KnownManagedMcpTool {
        connector: "tasks",
        tool_id: "run_now",
        qualified_name: "tasks__run_now",
        display_name: "Run Now",
        alias_english: "Tasks Run Now",
        description_sha256: "cb624d3983f786b70574c451502ea4e008f49ec30fb48d45f44288346e7dc4e5",
    },
    KnownManagedMcpTool {
        connector: "tasks",
        tool_id: "update",
        qualified_name: "tasks__update",
        display_name: "Update",
        alias_english: "Tasks Update",
        description_sha256: "49f0dc9572c909d58bcbf64f72f9e00740f606c658251fdc578704b31bba4fae",
    },
    KnownManagedMcpTool {
        connector: "tasks",
        tool_id: "validate",
        qualified_name: "tasks__validate",
        display_name: "Validate",
        alias_english: "Tasks Validate",
        description_sha256: "0f6673236d56ecef82e3bd08901c60a78ecb98b217dd5c3d2c7514d8490c364a",
    },
];

pub(crate) fn managed_connector_display_name(connector: &str) -> Option<&'static str> {
    match connector {
        "github" => Some("GitHub"),
        "gmail" => Some("Gmail"),
        "outlook" => Some("Outlook"),
        "tasks" => Some("Automations"),
        _ => None,
    }
}

pub(crate) fn known_managed_mcp_tool(
    connector: &str,
    qualified_name: &str,
    display_name: &str,
) -> Option<&'static KnownManagedMcpTool> {
    KNOWN_MANAGED_MCP_TOOLS.iter().find(|entry| {
        entry.connector == connector
            && entry.qualified_name == qualified_name
            && entry.display_name == display_name
    })
}

fn known_managed_mcp_tool_by_name(qualified_name: &str) -> Option<&'static KnownManagedMcpTool> {
    KNOWN_MANAGED_MCP_TOOLS
        .iter()
        .find(|entry| entry.qualified_name == qualified_name)
}

fn label_key(entry: &KnownManagedMcpTool) -> String {
    format!("scrollback.tool.mcp.{}.{}", entry.connector, entry.tool_id)
}

fn description_key(entry: &KnownManagedMcpTool) -> String {
    format!(
        "extensions.catalog.mcp.{}.{}.description",
        entry.connector, entry.tool_id
    )
}

fn identity_matches(entry: &KnownManagedMcpTool, identity: &ManagedGatewayToolIdentity) -> bool {
    identity.qualified_name == entry.qualified_name
        && identity.connector_id == entry.connector
        && identity.tool_id == entry.tool_id
        && identity.display_name == entry.display_name
        && identity.description_sha256 == entry.description_sha256
}

fn description_matches(entry: &KnownManagedMcpTool, raw: &str) -> bool {
    format!("{:x}", Sha256::digest(raw.as_bytes())) == entry.description_sha256
}

pub(crate) fn localized_verified_managed_mcp_tool_name(
    qualified_name: &str,
    identity: &ManagedGatewayToolIdentity,
    locale: &LocaleContext,
) -> Option<&'static str> {
    let entry = known_managed_mcp_tool_by_name(qualified_name)?;
    if !identity_matches(entry, identity) {
        return None;
    }
    let localized = locale.named_static_text(&label_key(entry), entry.alias_english);
    (localized != entry.alias_english).then_some(localized)
}

pub(crate) fn localized_managed_mcp_tool_label(
    entry: &KnownManagedMcpTool,
    locale: &LocaleContext,
) -> Option<&'static str> {
    let localized = locale.named_static_text(&label_key(entry), entry.alias_english);
    (localized != entry.alias_english).then_some(localized)
}

pub(crate) fn localized_managed_mcp_tool_description(
    entry: &KnownManagedMcpTool,
    raw: &str,
    locale: &LocaleContext,
) -> Option<String> {
    if !description_matches(entry, raw) {
        return None;
    }
    let localized = locale.named_text(&description_key(entry), raw).into_owned();
    (localized != raw).then_some(localized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locale::{LocaleSource, ResolvedLocale, UiLocale};
    use std::collections::HashSet;

    fn zh() -> LocaleContext {
        LocaleContext::new(ResolvedLocale {
            locale: UiLocale::ZhCn,
            source: LocaleSource::Cli,
        })
    }

    #[test]
    fn managed_mcp_catalog_is_complete_and_unique() {
        assert_eq!(KNOWN_MANAGED_MCP_TOOLS.len(), 137);
        assert_eq!(
            KNOWN_MANAGED_MCP_TOOLS
                .iter()
                .filter(|entry| entry.connector == "github")
                .count(),
            93
        );
        assert_eq!(
            KNOWN_MANAGED_MCP_TOOLS
                .iter()
                .filter(|entry| entry.connector == "gmail")
                .count(),
            16
        );
        assert_eq!(
            KNOWN_MANAGED_MCP_TOOLS
                .iter()
                .filter(|entry| entry.connector == "outlook")
                .count(),
            18
        );
        assert_eq!(
            KNOWN_MANAGED_MCP_TOOLS
                .iter()
                .filter(|entry| entry.connector == "tasks")
                .count(),
            10
        );

        let mut identities = HashSet::new();
        for entry in KNOWN_MANAGED_MCP_TOOLS {
            assert!(identities.insert((
                entry.connector,
                entry.tool_id,
                entry.qualified_name,
                entry.display_name,
            )));
            assert_eq!(
                entry.qualified_name,
                format!("{}__{}", entry.connector, entry.tool_id)
            );
            let identity = ManagedGatewayToolIdentity {
                qualified_name: entry.qualified_name.to_owned(),
                connector_id: entry.connector.to_owned(),
                tool_id: entry.tool_id.to_owned(),
                display_name: entry.display_name.to_owned(),
                description_sha256: entry.description_sha256.to_owned(),
            };
            assert!(
                localized_verified_managed_mcp_tool_name(entry.qualified_name, &identity, &zh())
                    .is_some(),
                "missing Chinese display label for {}",
                entry.qualified_name
            );
            let description_key = description_key(entry);
            assert_ne!(
                zh().named_text(&description_key, "__missing_description__"),
                "__missing_description__",
                "missing Chinese description for {}",
                entry.qualified_name
            );
        }
    }

    #[test]
    fn managed_mcp_description_requires_the_exact_pinned_copy() {
        let entry = known_managed_mcp_tool("github", "github__create_gist", "Create Gist")
            .expect("known GitHub tool");
        let locale = zh();
        assert!(
            localized_managed_mcp_tool_description(entry, "Create a new gist", &locale).is_some()
        );
        assert!(
            localized_managed_mcp_tool_description(entry, "Create a new gist.", &locale).is_none()
        );
    }

    #[test]
    fn unknown_or_mismatched_managed_tool_is_not_recognized() {
        assert!(known_managed_mcp_tool("github", "github__create_gist", "Create gist").is_none());
        assert!(known_managed_mcp_tool("custom", "github__create_gist", "Create Gist").is_none());

        let spoofed = ManagedGatewayToolIdentity {
            qualified_name: "github__create_gist".into(),
            connector_id: "custom".into(),
            tool_id: "create_gist".into(),
            display_name: "Create Gist".into(),
            description_sha256: "a1".repeat(32),
        };
        assert!(
            localized_verified_managed_mcp_tool_name("github__create_gist", &spoofed, &zh())
                .is_none()
        );

        let wrong_digest = ManagedGatewayToolIdentity {
            qualified_name: "github__create_gist".into(),
            connector_id: "github".into(),
            tool_id: "create_gist".into(),
            display_name: "Create Gist".into(),
            description_sha256: "00".repeat(32),
        };
        assert!(
            localized_verified_managed_mcp_tool_name("github__create_gist", &wrong_digest, &zh())
                .is_none()
        );
    }
}
