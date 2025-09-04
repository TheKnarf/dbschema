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
