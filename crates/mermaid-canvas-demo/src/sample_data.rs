//! 示例数据
//!
//! 提供各种 Mermaid 图表类型的示例源码字符串。

/// Flowchart 示例 — 包含多种节点形状、边标签、分支
pub fn flowchart_sample() -> &'static str {
    r#"flowchart TD
    A[Start] --> B{Choice?}
    B -->|yes| C[Action]
    B -->|no| D[(Database)]
    C --> E((End))
    D --> E"#
}

/// Class Diagram 示例 — 类继承关系与成员
pub fn class_sample() -> &'static str {
    r#"classDiagram
    Animal <|-- Dog
    Animal <|-- Cat
    Animal : +name: string
    Animal : +speak() string
    Dog : +bark() string
    Cat : +purr() string"#
}

/// State Diagram 示例 — 状态转换
pub fn state_sample() -> &'static str {
    r#"stateDiagram-v2
    [*] --> Idle
    Idle --> Running : start
    Running --> Paused : pause
    Paused --> Running : resume
    Running --> Idle : stop
    Idle --> [*]"#
}

/// ER Diagram 示例 — 实体关系
pub fn er_sample() -> &'static str {
    r#"erDiagram
    CUSTOMER ||--o{ ORDER : places
    ORDER ||--|{ LINE-ITEM : contains
    PRODUCT ||--o{ LINE-ITEM : "ordered in"
    CUSTOMER {
        string name
        int age
    }"#
}

/// Requirement Diagram 示例 — 需求追踪
pub fn requirement_sample() -> &'static str {
    r#"requirementDiagram
    requirement test_req {
        id: 1
        text: the test text
        risk: high
        verifymethod: test
    }
    element test_entity {
        type: simulation
    }
    test_entity - satisfies -> test_req"#
}

/// Packet Diagram 示例 — 网络数据包结构
pub fn packet_sample() -> &'static str {
    r#"packet
    0-7 : Source Port
    8-15 : Destination Port
    16-31 : Sequence Number
    32-47 : Acknowledgment Number"#
}

/// Sequence Diagram 示例 — 完整的序列图
pub fn sequence_sample() -> &'static str {
    r#"sequenceDiagram
    participant Client
    participant Server
    participant Database

    Client->>Server: HTTP Request
    activate Server
    Server->>Database: Query
    activate Database
    Database-->>Server: Results
    deactivate Database
    Server-->>Client: Response
    deactivate Server

    Note right of Server: Processing complete

    loop Periodic health check
        Client->>Server: Ping
        Server-->>Client: Pong
    end"#
}
