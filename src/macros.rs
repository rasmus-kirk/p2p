macro_rules! skip_fail {
    ($res:expr) => {
        match $res {
            Ok(val) => val,
            Err(e) => {
                warn!("An error: {}; skipped.", e);
                continue;
            }
        }
    };
}

macro_rules! verify_len {
    ($call:expr, $len_1:expr, $len_2:expr) => {
        if $len_1 != $len_2 {
            error!(
                "Incorrect number of arguments to '{}', must be {} but was {}",
                $call, $len_1, $len_2
            );
            continue;
        }
    };
}

pub(crate) use skip_fail;
