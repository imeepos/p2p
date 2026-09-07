//! task 状态机与 parts 单测（行数红线拆出）。
use crate::task::*;

#[test]
fn valid_transitions() {
    let mut t = Task::new("t1".into(), "a1".into(), Message::user_text("你好"));
    assert_eq!(t.state, TaskState::Submitted);
    t.transition(TaskState::Working).unwrap();
    t.transition(TaskState::Completed).unwrap();
    assert_eq!(t.state, TaskState::Completed);
}

#[test]
fn invalid_transitions_rejected() {
    let mut t = Task::new("t1".into(), "a1".into(), Message::user_text("你好"));
    assert!(
        t.transition(TaskState::Completed).is_err(),
        "submitted 直达 completed 非法"
    );
    t.transition(TaskState::Working).unwrap();
    t.transition(TaskState::Completed).unwrap();
    assert!(t.transition(TaskState::Failed).is_err(), "终态不可再迁移");
    assert!(t.transition(TaskState::Cancelled).is_err());
}

#[test]
fn rejected_from_submitted() {
    let mut t = Task::new("t1".into(), "a1".into(), Message::user_text("你好"));
    t.transition(TaskState::Rejected).unwrap();
}

#[test]
fn upload_validation() {
    // role=agent 上行拒
    let agent_msg = Message {
        role: Role::Agent,
        parts: vec![Part::Text(TextPart { text: "x".into() })],
    };
    assert_eq!(agent_msg.validate_upload(1024), Err(TaskError::RoleNotUser));
    // File 上行拒
    let file_msg = Message {
        role: Role::User,
        parts: vec![Part::File(FilePart {
            name: "a.txt".into(),
            mime_type: "text/plain".into(),
            bytes: "AA==".into(),
        })],
    };
    assert_eq!(
        file_msg.validate_upload(1024),
        Err(TaskError::PartNotUploadable)
    );
    // 超预算拒
    let big = Message::user_text("a".repeat(2048));
    assert_eq!(big.validate_upload(1024), Err(TaskError::InputOverCap));
    // 正常通过
    assert!(Message::user_text("hi").validate_upload(1024).is_ok());
}

#[test]
fn append_rules() {
    let mut t = Task::new("t1".into(), "a1".into(), Message::user_text("你好"));
    // submitted 态不可追加（未进入 working）
    assert_eq!(
        t.append_user(Message::user_text("再来")),
        Err(TaskError::MessageOutOfOrder)
    );
    t.transition(TaskState::Working).unwrap();
    // agent 消息先到：append_agent 允许
    t.append_agent(Message {
        role: Role::Agent,
        parts: vec![Part::Text(TextPart {
            text: "收到".into(),
        })],
    })
    .unwrap();
    t.append_user(Message::user_text("继续")).unwrap();
    assert_eq!(t.messages.len(), 3);
    assert_eq!(t.input_used, "你好".len() + "继续".len());
}

#[test]
fn input_cap_enforced() {
    let mut t = Task::new(
        "t1".into(),
        "a1".into(),
        Message::user_text("x".repeat(TASK_INPUT_CAP_BYTES)),
    );
    t.transition(TaskState::Working).unwrap();
    assert_eq!(
        t.append_user(Message::user_text("y")),
        Err(TaskError::InputOverCap)
    );
}
