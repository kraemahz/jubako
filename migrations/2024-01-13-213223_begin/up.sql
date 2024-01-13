CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

---- Projects
CREATE TABLE projects (
    id UUID PRIMARY KEY,
    name VARCHAR(100) UNIQUE,
    owner_id UUID NOT NULL REFERENCES auth.users(id),
    created TIMESTAMP NOT NULL,
    description VARCHAR NOT NULL
);

---- Repositories
CREATE TABLE repositories (
    id UUID PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    project_id UUID REFERENCES repositories(id),

    created TIMESTAMP NOT NULL,
    description TEXT NOT NULL,
    last_updated TIMESTAMP NOT NULL,

    main_branch VARCHAR(255) NOT NULL,
    head_ref CHAR(40) NOT NULL,

    UNIQUE (name, project_id)
);

CREATE TABLE repository_branches (
    repository_id UUID REFERENCES repositories(id),
    branch_name VARCHAR(255),

    head_ref CHAR(40) NOT NULL,
    branch_ref CHAR(40) NOT NULL,

    PRIMARY KEY (repository_id, branch_name)
);

CREATE TABLE repository_tags (
    repository_id UUID REFERENCES repositories(id),
    tag_name VARCHAR(255),
    ref CHAR(40) NOT NULL,

    PRIMARY KEY (repository_id, tag_name)
);

CREATE TABLE repository_parents (
    parent_repository_id UUID REFERENCES repositories(id),
    child_repository_id UUID REFERENCES repositories(id),
    PRIMARY KEY (parent_repository_id, child_repository_id)
);

CREATE TABLE repository_commits (
    ref CHAR(40) NOT NULL,
    repository_id UUID NOT NULL REFERENCES repositories(id),

    message TEXT,
    parent_ref CHAR(40),
    author_email VARCHAR NOT NULL,
    author_id UUID REFERENCES auth.users(id),
    created TIMESTAMP,

    PRIMARY KEY (ref, repository_id)
);

---- Pull Requests
CREATE TABLE pull_requests (
    id UUID PRIMARY KEY,

    repository_id UUID REFERENCES repositories(id),
    created TIMESTAMP NOT NULL,
    title VARCHAR(128) NOT NULL,
    description TEXT,

    from_repository UUID REFERENCES repositories(id),
    from_branch_name VARCHAR(255), 

    to_branch_name VARCHAR(255),

    FOREIGN KEY (from_repository, from_branch_name) REFERENCES repository_branches(repository_id, branch_name),
    FOREIGN KEY (repository_id, to_branch_name) REFERENCES repository_branches(repository_id, branch_name)
);

CREATE TABLE pull_request_comments (
    id UUID PRIMARY KEY,

    pull_request_id UUID REFERENCES pull_requests(id),
    author_id UUID NOT NULL REFERENCES auth.users(id),
    created TIMESTAMP NOT NULL,
    replying_to UUID REFERENCES pull_request_comments(id),
    content TEXT NOT NULL
);

CREATE TABLE file_comments (
    comment_id UUID PRIMARY KEY REFERENCES pull_request_comments(id),
    file_name VARCHAR(4096) NOT NULL,
    ref CHAR(40) NOT NULL,
    line_number INTEGER
);
