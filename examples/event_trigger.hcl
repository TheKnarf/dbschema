function "ddl_logger" {
  language = "plpgsql"
  returns  = "event_trigger"
  body = <<-SQL
    BEGIN
      -- handle ddl start
    END;
  SQL
}

event_trigger "log_ddl" {
  event = "ddl_command_start"
  tags  = ["CREATE TABLE"]
  function = "ddl_logger"
}

test "event_trigger_exists" {
  assert = [
    "SELECT EXISTS (SELECT 1 FROM pg_event_trigger WHERE evtname = 'log_ddl')"
  ]
  assert_fail = [
    "CREATE EVENT TRIGGER log_ddl ON ddl_command_start EXECUTE FUNCTION ddl_logger()"
  ]
}
